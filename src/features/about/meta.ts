import { Info } from "lucide-react";
import { lazy } from "react";

import type { SystemPageDefinition } from "@/features/types";

const about: SystemPageDefinition = {
  id: "about",
  route: "/about",
  titleKey: "features.about.title",
  descKey: "features.about.desc",
  icon: Info,
  order: 20,
  Page: lazy(() => import("./index")),
};

export default about;
