import { nullthrows } from "./utils/invariant";

export const PORT = nullthrows(process.env.PORT, "PORT is required");
