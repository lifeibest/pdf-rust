import { LayoutGrid } from "lucide-react";
import { lazy } from "react";

import { PDF_EXTENSIONS } from "@/features/accepts";
import type { FeatureDefinition } from "@/features/types";

const organize: FeatureDefinition = {
  id: "organize",
  route: "/organize",
  titleKey: "features.organize.title",
  descKey: "features.organize.desc",
  icon: LayoutGrid,
  accent: "from-violet-500 to-fuchsia-500",
  category: "organize",
  order: 22,
  accepts: { ext: [...PDF_EXTENSIONS], multiple: false },
  options: [],
  needsOutputDir: true,
  sortEnabled: false,
  thumbEnabled: true,
  mixedSources: false,
  Page: lazy(() => import("./index")),
};

export default organize;
