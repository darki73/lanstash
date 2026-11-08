/**
 * Preference: a choice this browser remembers for the page, kept in its local storage: yes or no
 * (preference: showing hidden games), or a word (choice: the language). Storage that is missing,
 * blocked or full is no error: the choice reads as no (or none) and is kept for the page's life
 * only. The storage is passed in.
 */

/** The part of the browser's storage this adapter uses. */
export interface PreferenceStorage {
  getItem(key: string): string | null
  setItem(key: string, value: string): void
  removeItem(key: string): void
}

export interface Preference {
  read(): boolean
  write(on: boolean): void
}

export function preference(storage: PreferenceStorage | undefined, key: string): Preference {
  return {
    read: () => {
      try {
        return storage?.getItem(key) === "yes"
      } catch {
        return false
      }
    },
    write: (on) => {
      try {
        if (on) storage?.setItem(key, "yes")
        else storage?.removeItem(key)
      } catch {
        // Not kept: the choice holds for this page only.
      }
    },
  }
}

export interface Choice {
  read(): string | undefined
  /** Keeps `value`; undefined forgets the choice. */
  write(value: string | undefined): void
}

export function choice(storage: PreferenceStorage | undefined, key: string): Choice {
  return {
    read: () => {
      try {
        return storage?.getItem(key) ?? undefined
      } catch {
        return undefined
      }
    },
    write: (value) => {
      try {
        if (value === undefined) storage?.removeItem(key)
        else storage?.setItem(key, value)
      } catch {
        // Not kept: the choice holds for this page only.
      }
    },
  }
}
