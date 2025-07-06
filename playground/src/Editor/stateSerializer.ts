import { strFromU8, strToU8, unzlibSync, zlibSync } from "fflate";

// https://github.com/vuejs/repl/blob/a93c8002b6a677cb255dbad91274e8f91ec3438e/src/utils.ts
export function toBase64(data: string): string {
  const buffer = strToU8(data);
  const zipped = zlibSync(buffer, { level: 9 });
  const binary = strFromU8(zipped, true);
  return btoa(binary);
}

export function fromBase64(base64: string): string {
  const binary = atob(base64);

  const buffer = strToU8(binary, true);
  const unzipped = unzlibSync(buffer);
  return strFromU8(unzipped);
}
