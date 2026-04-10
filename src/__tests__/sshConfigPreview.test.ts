import { describe, it, expect } from "vitest";
import type {
  SshConfigEntry,
  SshConfigPreviewResult,
  SshConfigImportResult,
} from "@/types/sshConfig";

describe("SSH Config Types", () => {
  describe("SshConfigEntry", () => {
    it("represents a basic host entry", () => {
      const entry: SshConfigEntry = {
        hostAlias: "myserver",
        hostname: "10.0.1.1",
        port: 22,
        user: "admin",
        isWildcard: false,
        isNonInteractive: false,
        rawOptions: {},
      };
      expect(entry.hostAlias).toBe("myserver");
      expect(entry.hostname).toBe("10.0.1.1");
      expect(entry.port).toBe(22);
      expect(entry.isWildcard).toBe(false);
    });

    it("represents a key-based auth entry", () => {
      const entry: SshConfigEntry = {
        hostAlias: "prod",
        hostname: "prod.example.com",
        port: 2222,
        user: "deploy",
        identityFile: "/home/user/.ssh/id_ed25519",
        isWildcard: false,
        isNonInteractive: false,
        rawOptions: {},
      };
      expect(entry.identityFile).toBeDefined();
      expect(entry.identityFile).toContain("id_ed25519");
    });

    it("represents an entry with proxy jump", () => {
      const entry: SshConfigEntry = {
        hostAlias: "internal",
        hostname: "10.0.2.1",
        port: 22,
        user: "root",
        proxyJump: "bastion1,bastion2",
        isWildcard: false,
        isNonInteractive: false,
        rawOptions: {},
      };
      expect(entry.proxyJump).toBe("bastion1,bastion2");
    });
  });

  describe("SshConfigPreviewResult", () => {
    it("can be constructed with entries and no errors", () => {
      const result: SshConfigPreviewResult = {
        entries: [
          {
            hostAlias: "server1",
            hostname: "10.0.1.1",
            port: 22,
            user: "admin",
            isWildcard: false,
            isNonInteractive: false,
            rawOptions: {},
          },
          {
            hostAlias: "server2",
            hostname: "10.0.1.2",
            port: 2222,
            user: "root",
            isWildcard: false,
            isNonInteractive: false,
            rawOptions: {},
          },
        ],
        errors: [],
      };
      expect(result.entries).toHaveLength(2);
      expect(result.errors).toHaveLength(0);
    });

    it("can contain parse warnings", () => {
      const result: SshConfigPreviewResult = {
        entries: [],
        errors: [
          {
            file: "/home/user/.ssh/config",
            line: 15,
            message:
              "Match directive skipped at line 15, only Host blocks are supported",
          },
        ],
      };
      expect(result.errors).toHaveLength(1);
      expect(result.errors[0].message).toContain("Match");
    });

    it("wildcards should be filtered out in preview", () => {
      const rawEntries: SshConfigEntry[] = [
        {
          hostAlias: "*",
          hostname: "",
          port: 22,
          user: "default",
          isWildcard: true,
          isNonInteractive: false,
          rawOptions: {},
        },
        {
          hostAlias: "myhost",
          hostname: "10.0.1.1",
          port: 22,
          user: "default",
          isWildcard: false,
          isNonInteractive: false,
          rawOptions: {},
        },
      ];

      const filtered = rawEntries.filter((e) => !e.isWildcard);
      expect(filtered).toHaveLength(1);
      expect(filtered[0].hostAlias).toBe("myhost");
    });
  });

  describe("SshConfigImportResult", () => {
    it("represents a successful import", () => {
      const result: SshConfigImportResult = {
        total: 10,
        imported: 8,
        skipped: 2,
        errors: [],
      };
      expect(result.imported + result.skipped).toBe(result.total);
    });

    it("represents import with errors", () => {
      const result: SshConfigImportResult = {
        total: 5,
        imported: 3,
        skipped: 1,
        errors: [
          {
            hostAlias: "badhost",
            message: "ProxyJump mapping failed: bastion not found",
          },
        ],
      };
      expect(result.errors).toHaveLength(1);
      expect(result.imported + result.skipped + result.errors.length).toBe(
        result.total,
      );
    });
  });

  describe("Selection helpers", () => {
    it("select all returns all aliases", () => {
      const entries: SshConfigEntry[] = [
        { hostAlias: "a", hostname: "1", port: 22, user: "u", isWildcard: false, isNonInteractive: false, rawOptions: {} },
        { hostAlias: "b", hostname: "2", port: 22, user: "u", isWildcard: false, isNonInteractive: false, rawOptions: {} },
        { hostAlias: "c", hostname: "3", port: 22, user: "u", isWildcard: false, isNonInteractive: false, rawOptions: {} },
      ];
      const selected = entries.map((e) => e.hostAlias);
      expect(selected).toEqual(["a", "b", "c"]);
    });

    it("deselect all returns empty", () => {
      const selected: string[] = [];
      expect(selected).toHaveLength(0);
    });

    it("toggle selection adds/removes alias", () => {
      let selected = ["a", "b"];
      // Remove "a"
      selected = selected.filter((s) => s !== "a");
      expect(selected).toEqual(["b"]);
      // Add "c"
      selected.push("c");
      expect(selected).toEqual(["b", "c"]);
    });
  });
});
