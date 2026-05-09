export type DefaultPlayMode = "cinematic" | "fullscreen" | "pip";
export type EndBehavior = "nothing" | "loop" | "next";

export function getDefaultPlayMode(raw?: string | null): DefaultPlayMode {
  return raw === "fullscreen" || raw === "pip" ? raw : "cinematic";
}

export function getEndBehavior(raw?: string | null): EndBehavior {
  return raw === "loop" || raw === "next" ? raw : "nothing";
}
