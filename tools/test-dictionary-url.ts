import { readFileSync } from "node:fs";

export const offlineDictionaryUrl = Bun.TOML.parse(readFileSync(new URL("../.cargo/config.toml", import.meta.url), "utf8")).env?.PROVENANCE_TEST_STE100_ASSET_URL?.value;
if (typeof offlineDictionaryUrl !== "string" || !offlineDictionaryUrl.startsWith("http://127.0.0.1:")) {
  throw new Error(".cargo/config.toml: env.PROVENANCE_TEST_STE100_ASSET_URL.value must be a string that starts with http://127.0.0.1:");
}
