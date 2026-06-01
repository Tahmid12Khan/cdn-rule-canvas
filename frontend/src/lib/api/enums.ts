import { z } from "zod";

// Wire forms are lowercase/snake per BACKEND CONTRACT §2. Status pills are
// uppercased only for display.
export const FeatureType = z.enum(["html", "json"]);
export type FeatureType = z.infer<typeof FeatureType>;

export const VersionStatus = z.enum(["draft", "staging", "live", "prev"]);
export type VersionStatus = z.infer<typeof VersionStatus>;

export const Placement = z.enum(["inline", "sticky_footer", "popup"]);
export type Placement = z.infer<typeof Placement>;

export const PublishEnvironment = z.enum(["staging", "live"]);
export type PublishEnvironment = z.infer<typeof PublishEnvironment>;
