export type AppInfo = {
  productName: string;
  binaryName: string;
  version: string;
  storagePath: string;
};

export const fallbackAppInfo: AppInfo = {
  productName: "rDeckForge",
  binaryName: "rdeckforge",
  version: "0.1.0",
  storagePath: "~/Library/Application Support/rDeckForge/rdeckforge.sqlite3",
};
