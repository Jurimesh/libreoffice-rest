import { nullthrows } from "./utils/invariant";

export const PORT = +nullthrows(process.env.PORT, "PORT is required");
export const TARGET_DIR = nullthrows(process.env.TARGET_DIR, "TARGET_DIR is required");
