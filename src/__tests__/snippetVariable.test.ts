import { describe, it, expect } from "vitest";

/**
 * Tests for snippet variable template parsing.
 * These test the frontend-side extraction logic (mirror of Rust implementation).
 * The actual resolve happens on the backend via snippet_execute.
 */

/** Extract ${VAR_NAME} variable names from a command template. */
function extractVariables(command: string): string[] {
  const vars: string[] = [];
  const seen = new Set<string>();
  const regex = /\$\{([^}]+)\}/g;
  let match;
  while ((match = regex.exec(command)) !== null) {
    const name = match[1];
    if (!seen.has(name)) {
      seen.add(name);
      vars.push(name);
    }
  }
  return vars;
}

describe("Snippet Variable Templates", () => {
  describe("extractVariables", () => {
    it("extracts single variable", () => {
      expect(extractVariables("kubectl apply -f ${FILE}")).toEqual(["FILE"]);
    });

    it("extracts multiple variables in order", () => {
      expect(
        extractVariables("scp ${USER}@${HOST}:${PATH} ."),
      ).toEqual(["USER", "HOST", "PATH"]);
    });

    it("deduplicates repeated variables", () => {
      expect(
        extractVariables("echo ${X} and ${X} again"),
      ).toEqual(["X"]);
    });

    it("returns empty for no variables", () => {
      expect(extractVariables("ls -la /tmp")).toEqual([]);
    });

    it("handles adjacent variables", () => {
      expect(
        extractVariables("${PREFIX}${SUFFIX}"),
      ).toEqual(["PREFIX", "SUFFIX"]);
    });

    it("handles variables with underscores and numbers", () => {
      expect(
        extractVariables("echo ${MY_VAR_1} ${another_2}"),
      ).toEqual(["MY_VAR_1", "another_2"]);
    });

    it("ignores unclosed braces", () => {
      expect(extractVariables("echo ${OPEN")).toEqual([]);
    });

    it("ignores dollar without brace", () => {
      expect(extractVariables("echo $HOME")).toEqual([]);
    });
  });

  describe("variable resolution (simulated)", () => {
    function resolveVariables(
      command: string,
      vars: Record<string, string>,
    ): string {
      return command.replace(/\$\{([^}]+)\}/g, (match, name) => {
        return vars[name] ?? match;
      });
    }

    it("resolves all variables", () => {
      expect(
        resolveVariables("ssh ${USER}@${HOST}", {
          USER: "admin",
          HOST: "10.0.1.1",
        }),
      ).toBe("ssh admin@10.0.1.1");
    });

    it("leaves unresolved variables as-is", () => {
      expect(
        resolveVariables("ssh ${USER}@${HOST}", { USER: "admin" }),
      ).toBe("ssh admin@${HOST}");
    });

    it("handles empty variables map", () => {
      expect(resolveVariables("echo ${X}", {})).toBe("echo ${X}");
    });

    it("handles command with no variables", () => {
      expect(resolveVariables("ls -la", { X: "y" })).toBe("ls -la");
    });
  });
});
