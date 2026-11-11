import { KeyRound } from "lucide-react";
import { lazy } from "react";

import type { SystemPageDefinition } from "@/features/types";

const activate: SystemPageDefinition = {
  id: "activate",
  route: "/activate",
  titleKey: "features.activate.title",
  descKey: "features.activate.desc",
  icon: KeyRound,
  order: 5,
  Page: lazy(() => import("./index")),
};

export default activate;
