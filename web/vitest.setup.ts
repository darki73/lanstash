/**
 * Test setup: unmounts what each test drew.
 */
import { cleanup } from "@testing-library/react"
import { afterEach } from "vitest"

afterEach(() => cleanup())
