/**
 * Class names: joins the class names that are set, skipping false, null, undefined and empty
 * strings. Tailwind conflicts are not merged; each property is set in one place instead.
 */
export type ClassValue = string | false | null | undefined

export function classes(...values: readonly ClassValue[]): string {
  return values.filter((value): value is string => typeof value === "string" && value !== "").join(" ")
}
