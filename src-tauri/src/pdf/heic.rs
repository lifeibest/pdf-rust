//! HEIC / HEIF 图片解码（平台分治方案）。
//!
//! - **macOS**：调用系统内置 `/usr/bin/sips`（底层为 CoreGraphics / ImageIO），
//!   零额外体积、无需处理 Universal 双架构 dylib 与签名，天然支持 Apple 10-bit / HDR /
//!   Display P3 与 EXIF/iPhone 旋转方向。
//! - **Windows**：运行时动态加载随包分发的 `heif.dll`（及其依赖 `libde265.dll`），
//!   完全离线可用、不依赖微软商店收费的 HEVC 视频扩展；动态链接方式满足 LGPL-3.0 合规。
//!   通过 `heif_context_read_from_memory_without_copy` 传内存缓冲，彻底避开 Windows C 接口
//!   的中文路径编码问题。

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use image::DynamicImage;

use crate::error::AppError;
use crate::pdf::imaging;

/// 生产环境的额外库搜索目录（通常是 Tauri 的 `resource_dir`），由 `lib.rs::setup` 注入。
static EXTRA_DIRS: OnceLock<Vec<PathBuf>> = OnceLock::new();

/// 记录运行时搜索目录（Windows 用于定位 `heif.dll`；其他平台无副作用）。
pub fn init_extra_dirs(extra_dirs: &[PathBuf]) {
    let _ = EXTRA_DIRS.set(extra_dirs.to_vec());
}

/// 判断路径是否为 HEIC / HEIF 图片。
pub fn is_heic(path: &Path) -> bool {
    matches!(imaging::extension_of(path).as_str(), "heic" | "heif")
}

/// 解码 HEIC / HEIF 图片为 `DynamicImage`。
pub fn decode(path: &Path) -> Result<DynamicImage, AppError> {
    if !path.exists() {
        return Err(AppError::Io(format!("图片不存在：{}", path.display())));
    }

    decode_platform(path)
}

#[cfg(target_os = "macos")]
fn decode_platform(path: &Path) -> Result<DynamicImage, AppError> {
    use std::process::Command;

    struct TempFileGuard(PathBuf);
    impl Drop for TempFileGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    let temp_png = std::env::temp_dir().join(format!(
        "pdfzt-heic-{}.png",
        uuid::Uuid::new_v4().simple()
    ));
    let _guard = TempFileGuard(temp_png.clone());

    let output = Command::new("/usr/bin/sips")
        .args(["-s", "format", "png"])
        .arg(path)
        .arg("--out")
        .arg(&temp_png)
        .output()
        .map_err(|err| AppError::Image(format!("调用系统 sips 解码 HEIC 失败: {err}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr.trim();
        return Err(AppError::Image(format!(
            "无法解码 HEIC 图片 {}: {}",
            path.display(),
            if detail.is_empty() {
                "sips 返回非零状态"
            } else {
                detail
            }
        )));
    }

    image::open(&temp_png)
        .map_err(|err| AppError::Image(format!("无法读取 HEIC 转换结果 {}: {err}", path.display())))
}

#[cfg(windows)]
fn decode_platform(path: &Path) -> Result<DynamicImage, AppError> {
    windows_libheif::decode(path)
}

#[cfg(not(any(target_os = "macos", windows)))]
fn decode_platform(path: &Path) -> Result<DynamicImage, AppError> {
    Err(AppError::Unsupported(format!(
        "当前平台暂不支持解码 HEIC/HEIF 图片：{}",
        path.display()
    )))
}

#[cfg(windows)]
mod windows_libheif {
    use std::ffi::{CStr, c_char, c_int, c_void};
    use std::path::{Path, PathBuf};
    use std::ptr;
    use std::sync::OnceLock;

    use image::{DynamicImage, RgbaImage};
    use libloading::Library;
    use libloading::os::windows::Library as WinLibrary;

    use super::EXTRA_DIRS;
    use crate::error::AppError;

    // Win32 LoadLibraryExW 标志：优先在目标 DLL 所在目录搜索其依赖（如 libde265.dll）
    const LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR: u32 = 0x0000_0100;
    const LOAD_LIBRARY_SEARCH_DEFAULT_DIRS: u32 = 0x0000_1000;

    // libheif C ABI 枚举常量（来自 libheif/heif.h）
    const HEIF_ERROR_OK: c_int = 0;
    const HEIF_COLORSPACE_RGB: c_int = 1;
    const HEIF_CHROMA_INTERLEAVED_RGBA: c_int = 11;
    const HEIF_CHANNEL_INTERLEAVED: c_int = 10;

    #[repr(C)]
    struct HeifError {
        code: c_int,
        subcode: c_int,
        message: *const c_char,
    }

    enum HeifContext {}
    enum HeifImageHandle {}
    enum HeifImage {}

    type FnHeifInit = unsafe extern "C" fn(*mut c_void) -> HeifError;
    type FnContextAlloc = unsafe extern "C" fn() -> *mut HeifContext;
    type FnContextFree = unsafe extern "C" fn(*mut HeifContext);
    type FnReadFromMemory = unsafe extern "C" fn(
        *mut HeifContext,
        *const c_void,
        usize,
        *const c_void,
    ) -> HeifError;
    type FnGetPrimaryHandle =
        unsafe extern "C" fn(*mut HeifContext, *mut *mut HeifImageHandle) -> HeifError;
    type FnReleaseHandle = unsafe extern "C" fn(*const HeifImageHandle);
    type FnDecodeImage = unsafe extern "C" fn(
        *const HeifImageHandle,
        *mut *mut HeifImage,
        c_int,
        c_int,
        *const c_void,
    ) -> HeifError;
    type FnImageGetWidth = unsafe extern "C" fn(*const HeifImage, c_int) -> c_int;
    type FnImageGetHeight = unsafe extern "C" fn(*const HeifImage, c_int) -> c_int;
    type FnImageGetPlaneReadonly =
        unsafe extern "C" fn(*const HeifImage, c_int, *mut c_int) -> *const u8;
    type FnReleaseImage = unsafe extern "C" fn(*const HeifImage);

    struct LibHeif {
        // 持有 Library 防止 DLL 被卸载
        _lib: Library,
        context_alloc: FnContextAlloc,
        context_free: FnContextFree,
        read_from_memory: FnReadFromMemory,
        get_primary_handle: FnGetPrimaryHandle,
        release_handle: FnReleaseHandle,
        decode_image: FnDecodeImage,
        image_get_width: FnImageGetWidth,
        image_get_height: FnImageGetHeight,
        image_get_plane_readonly: FnImageGetPlaneReadonly,
        release_image: FnReleaseImage,
    }

    // 函数指针与 DLL 句柄在进程生命周期内常驻，跨线程只读安全
    unsafe impl Send for LibHeif {}
    unsafe impl Sync for LibHeif {}

    static LIBHEIF: OnceLock<Result<LibHeif, String>> = OnceLock::new();

    fn get_lib() -> Result<&'static LibHeif, AppError> {
        let state = LIBHEIF.get_or_init(load_libheif);
        match state {
            Ok(lib) => Ok(lib),
            Err(msg) => Err(AppError::Image(msg.clone())),
        }
    }

    fn load_libheif() -> Result<LibHeif, String> {
        let mut candidates: Vec<PathBuf> = Vec::new();
        let names = ["heif.dll", "libheif.dll"];

        // 1. 环境变量覆盖，便于本地开发调试
        if let Ok(explicit) = std::env::var("LIBHEIF_PATH") {
            let p = PathBuf::from(&explicit);
            if p.is_dir() {
                for name in names {
                    candidates.push(p.join(name));
                }
            } else {
                candidates.push(p);
            }
        }

        // 2. 生产环境 resource_dir（Tauri 打包目录）
        if let Some(dirs) = EXTRA_DIRS.get() {
            for dir in dirs {
                for name in names {
                    candidates.push(dir.join(name));
                }
            }
        }

        // 3. 开发期 src-tauri/binaries/heif/ 与 src-tauri/binaries/
        let manifest_binaries = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries");
        for dir in [manifest_binaries.join("heif"), manifest_binaries] {
            for name in names {
                candidates.push(dir.join(name));
            }
        }

        // 4. 可执行文件同级目录（NSIS 安装目录）
        if let Some(exe_dir) = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(PathBuf::from))
        {
            for name in names {
                candidates.push(exe_dir.join(name));
            }
        }

        let mut errors: Vec<String> = Vec::new();
        let mut loaded: Option<Library> = None;

        for path in &candidates {
            if !path.exists() {
                continue;
            }
            let flags = LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS;
            match unsafe { WinLibrary::load_with_flags(path, flags) } {
                Ok(win_lib) => {
                    log::info!("libheif 加载成功: {}", path.display());
                    loaded = Some(Library::from(win_lib));
                    break;
                }
                Err(err) => {
                    errors.push(format!("{}: {err}", path.display()));
                }
            }
        }

        // 5. 兜底：系统 PATH
        if loaded.is_none() {
            for name in names {
                if let Ok(lib) = unsafe { Library::new(name) } {
                    log::info!("libheif 加载成功: 系统 {name}");
                    loaded = Some(lib);
                    break;
                }
            }
        }

        let lib = loaded.ok_or_else(|| {
            if errors.is_empty() {
                "未找到 HEIC 解码库（heif.dll / libde265.dll），无法解码 HEIC/HEIF 图片".to_string()
            } else {
                format!(
                    "加载 HEIC 解码库（heif.dll）失败：{}",
                    errors.join("; ")
                )
            }
        })?;

        unsafe {
            // libheif >= 1.13 提供 heif_init；存在时调用一次以初始化插件解码器
            if let Ok(init_fn) = lib.get::<FnHeifInit>(b"heif_init\0") {
                let err = init_fn(ptr::null_mut());
                if err.code != HEIF_ERROR_OK {
                    log::warn!("heif_init 返回非零状态: {}", format_heif_error(&err));
                }
            }

            macro_rules! load_sym {
                ($name:literal, $ty:ty) => {
                    *lib.get::<$ty>($name)
                        .map_err(|e| format!("heif.dll 缺少符号 {:?}: {e}", $name))?
                };
            }

            Ok(LibHeif {
                context_alloc: load_sym!(b"heif_context_alloc\0", FnContextAlloc),
                context_free: load_sym!(b"heif_context_free\0", FnContextFree),
                read_from_memory: load_sym!(
                    b"heif_context_read_from_memory_without_copy\0",
                    FnReadFromMemory
                ),
                get_primary_handle: load_sym!(
                    b"heif_context_get_primary_image_handle\0",
                    FnGetPrimaryHandle
                ),
                release_handle: load_sym!(b"heif_image_handle_release\0", FnReleaseHandle),
                decode_image: load_sym!(b"heif_decode_image\0", FnDecodeImage),
                image_get_width: load_sym!(b"heif_image_get_width\0", FnImageGetWidth),
                image_get_height: load_sym!(b"heif_image_get_height\0", FnImageGetHeight),
                image_get_plane_readonly: load_sym!(
                    b"heif_image_get_plane_readonly\0",
                    FnImageGetPlaneReadonly
                ),
                release_image: load_sym!(b"heif_image_release\0", FnReleaseImage),
                _lib: lib,
            })
        }
    }

    fn format_heif_error(err: &HeifError) -> String {
        if err.message.is_null() {
            format!("code={}, subcode={}", err.code, err.subcode)
        } else {
            let msg = unsafe { CStr::from_ptr(err.message) }.to_string_lossy();
            format!("{msg} (code={}, subcode={})", err.code, err.subcode)
        }
    }

    struct ContextGuard<'a> {
        lib: &'a LibHeif,
        ptr: *mut HeifContext,
    }
    impl Drop for ContextGuard<'_> {
        fn drop(&mut self) {
            if !self.ptr.is_null() {
                unsafe { (self.lib.context_free)(self.ptr) };
            }
        }
    }

    struct HandleGuard<'a> {
        lib: &'a LibHeif,
        ptr: *mut HeifImageHandle,
    }
    impl Drop for HandleGuard<'_> {
        fn drop(&mut self) {
            if !self.ptr.is_null() {
                unsafe { (self.lib.release_handle)(self.ptr) };
            }
        }
    }

    struct ImageGuard<'a> {
        lib: &'a LibHeif,
        ptr: *mut HeifImage,
    }
    impl Drop for ImageGuard<'_> {
        fn drop(&mut self) {
            if !self.ptr.is_null() {
                unsafe { (self.lib.release_image)(self.ptr) };
            }
        }
    }

    pub fn decode(path: &Path) -> Result<DynamicImage, AppError> {
        let lib = get_lib()?;
        let bytes = std::fs::read(path)?;

        let ctx_ptr = unsafe { (lib.context_alloc)() };
        if ctx_ptr.is_null() {
            return Err(AppError::Image("无法分配 libheif 上下文".to_string()));
        }
        let ctx = ContextGuard { lib, ptr: ctx_ptr };

        let err = unsafe {
            (lib.read_from_memory)(
                ctx.ptr,
                bytes.as_ptr().cast::<c_void>(),
                bytes.len(),
                ptr::null(),
            )
        };
        if err.code != HEIF_ERROR_OK {
            return Err(AppError::Image(format!(
                "无法解析 HEIC 文件 {}: {}",
                path.display(),
                format_heif_error(&err)
            )));
        }

        let mut handle_ptr: *mut HeifImageHandle = ptr::null_mut();
        let err = unsafe { (lib.get_primary_handle)(ctx.ptr, &mut handle_ptr) };
        if err.code != HEIF_ERROR_OK || handle_ptr.is_null() {
            return Err(AppError::Image(format!(
                "无法获取 HEIC 主图像 {}: {}",
                path.display(),
                format_heif_error(&err)
            )));
        }
        let handle = HandleGuard {
            lib,
            ptr: handle_ptr,
        };

        let mut img_ptr: *mut HeifImage = ptr::null_mut();
        let err = unsafe {
            (lib.decode_image)(
                handle.ptr,
                &mut img_ptr,
                HEIF_COLORSPACE_RGB,
                HEIF_CHROMA_INTERLEAVED_RGBA,
                ptr::null(),
            )
        };
        if err.code != HEIF_ERROR_OK || img_ptr.is_null() {
            return Err(AppError::Image(format!(
                "无法解码 HEIC 像素数据 {}: {}",
                path.display(),
                format_heif_error(&err)
            )));
        }
        let img = ImageGuard { lib, ptr: img_ptr };

        let width = unsafe { (lib.image_get_width)(img.ptr, HEIF_CHANNEL_INTERLEAVED) };
        let height = unsafe { (lib.image_get_height)(img.ptr, HEIF_CHANNEL_INTERLEAVED) };
        if width <= 0 || height <= 0 {
            return Err(AppError::Image(format!(
                "HEIC 图像尺寸无效 ({width}x{height}): {}",
                path.display()
            )));
        }
        let width = width as u32;
        let height = height as u32;

        let mut stride: c_int = 0;
        let plane =
            unsafe { (lib.image_get_plane_readonly)(img.ptr, HEIF_CHANNEL_INTERLEAVED, &mut stride) };
        let row_bytes = (width as usize)
            .checked_mul(4)
            .ok_or_else(|| AppError::Image(format!("HEIC 图像宽度溢出: {}", path.display())))?;
        if plane.is_null() || stride <= 0 || (stride as usize) < row_bytes {
            return Err(AppError::Image(format!(
                "HEIC 图像平面数据无效 (stride={stride}): {}",
                path.display()
            )));
        }
        let stride = stride as usize;

        let mut pixels = Vec::with_capacity(row_bytes * (height as usize));
        for y in 0..(height as usize) {
            let row = unsafe { std::slice::from_raw_parts(plane.add(y * stride), row_bytes) };
            pixels.extend_from_slice(row);
        }

        let rgba = RgbaImage::from_raw(width, height, pixels)
            .ok_or_else(|| AppError::Image(format!("构造 RGBA 位图失败: {}", path.display())))?;
        Ok(DynamicImage::ImageRgba8(rgba))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_heic_and_heif_case_insensitively() {
        assert!(is_heic(Path::new("/tmp/IMG_0001.HEIC")));
        assert!(is_heic(Path::new("/tmp/photo.heic")));
        assert!(is_heic(Path::new("/tmp/photo.heif")));
        assert!(is_heic(Path::new("/tmp/photo.HEIF")));
        assert!(!is_heic(Path::new("/tmp/photo.jpg")));
        assert!(!is_heic(Path::new("/tmp/doc.pdf")));
    }
}
