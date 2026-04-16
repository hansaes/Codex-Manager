export const ROOT_PAGE_PATHS = [
  "/",
  "/accounts",
  "/teams",
  "/aggregate-api",
  "/apikeys",
  "/models",
  "/plugins",
  "/logs",
  "/settings",
  "/author",
] as const;

export type RootPagePath = (typeof ROOT_PAGE_PATHS)[number];
