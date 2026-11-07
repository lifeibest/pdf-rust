import { Settings as SettingsIcon } from "lucide-react";
import { lazy } from "react";

import type { SystemPageDefinition } from "@/features/types";

const settings: SystemPageDefinition = {
  id: "settings",
  route: "/settings",
  titleKey: "features.settings.title",
  descKey: "features.settings.desc",
  icon: SettingsIcon,
  order: 10,
  Page: lazy(() => import("./index")),
};

export default settings;
