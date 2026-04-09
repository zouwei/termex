import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { useSnippetStore } from "@/stores/snippetStore";
import type { Snippet, SnippetFolder } from "@/types/snippet";

// Mock tauriInvoke
const mockInvoke = vi.fn();
vi.mock("@/utils/tauri", () => ({
  tauriInvoke: (...args: unknown[]) => mockInvoke(...args),
}));

const MOCK_SNIPPET: Snippet = {
  id: "s1",
  title: "Deploy",
  description: "Deploy to production",
  command: "kubectl apply -f ${FILE}",
  tags: ["k8s", "deploy"],
  folderId: "f1",
  isFavorite: true,
  usageCount: 5,
  lastUsedAt: "2026-04-09T00:00:00Z",
  createdAt: "2026-04-01T00:00:00Z",
  updatedAt: "2026-04-09T00:00:00Z",
};

const MOCK_FOLDER: SnippetFolder = {
  id: "f1",
  name: "Kubernetes",
  sortOrder: 0,
  createdAt: "2026-04-01T00:00:00Z",
};

describe("snippetStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    mockInvoke.mockReset();
  });

  it("loadSnippets calls snippet_list", async () => {
    mockInvoke.mockResolvedValueOnce([MOCK_SNIPPET]);
    const store = useSnippetStore();
    await store.loadSnippets();
    expect(mockInvoke).toHaveBeenCalledWith("snippet_list", {
      folderId: null,
      search: null,
    });
    expect(store.snippets).toHaveLength(1);
    expect(store.snippets[0].title).toBe("Deploy");
  });

  it("loadFolders calls snippet_folder_list", async () => {
    mockInvoke.mockResolvedValueOnce([MOCK_FOLDER]);
    const store = useSnippetStore();
    await store.loadFolders();
    expect(mockInvoke).toHaveBeenCalledWith("snippet_folder_list");
    expect(store.folders).toHaveLength(1);
    expect(store.folders[0].name).toBe("Kubernetes");
  });

  it("createSnippet calls snippet_create and reloads", async () => {
    const input = {
      title: "Test",
      command: "echo test",
      tags: [],
      isFavorite: false,
    };
    mockInvoke
      .mockResolvedValueOnce({ ...MOCK_SNIPPET, title: "Test" }) // create
      .mockResolvedValueOnce([]); // reload
    const store = useSnippetStore();
    const result = await store.createSnippet(input);
    expect(mockInvoke).toHaveBeenCalledWith("snippet_create", { input });
    expect(result.title).toBe("Test");
  });

  it("deleteSnippet calls snippet_delete and reloads", async () => {
    mockInvoke
      .mockResolvedValueOnce(undefined) // delete
      .mockResolvedValueOnce([]); // reload
    const store = useSnippetStore();
    await store.deleteSnippet("s1");
    expect(mockInvoke).toHaveBeenCalledWith("snippet_delete", { id: "s1" });
  });

  it("executeSnippet calls snippet_execute and reloads", async () => {
    mockInvoke
      .mockResolvedValueOnce(undefined) // execute
      .mockResolvedValueOnce([]); // reload
    const store = useSnippetStore();
    await store.executeSnippet("s1", "session-1", { FILE: "deploy.yaml" });
    expect(mockInvoke).toHaveBeenCalledWith("snippet_execute", {
      id: "s1",
      sessionId: "session-1",
      variables: { FILE: "deploy.yaml" },
    });
  });

  it("filteredSnippets filters by search query", async () => {
    mockInvoke.mockResolvedValueOnce([
      MOCK_SNIPPET,
      { ...MOCK_SNIPPET, id: "s2", title: "Restart", description: "Restart services", command: "systemctl restart", tags: ["ops"] },
    ]);
    const store = useSnippetStore();
    await store.loadSnippets();

    store.searchQuery = "deploy";
    expect(store.filteredSnippets).toHaveLength(1);
    expect(store.filteredSnippets[0].title).toBe("Deploy");
  });

  it("filteredSnippets filters by folder", async () => {
    mockInvoke.mockResolvedValueOnce([
      MOCK_SNIPPET,
      { ...MOCK_SNIPPET, id: "s2", title: "Other", folderId: "f2" },
    ]);
    const store = useSnippetStore();
    await store.loadSnippets();

    store.currentFolderId = "f1";
    expect(store.filteredSnippets).toHaveLength(1);
    expect(store.filteredSnippets[0].id).toBe("s1");
  });

  it("setFolder changes currentFolderId", () => {
    const store = useSnippetStore();
    expect(store.currentFolderId).toBeNull();
    store.setFolder("f1");
    expect(store.currentFolderId).toBe("f1");
    store.setFolder(null);
    expect(store.currentFolderId).toBeNull();
  });

  it("extractVariables calls snippet_extract_variables", async () => {
    mockInvoke.mockResolvedValueOnce(["FILE", "NS"]);
    const store = useSnippetStore();
    const vars = await store.extractVariables("kubectl -n ${NS} apply -f ${FILE}");
    expect(mockInvoke).toHaveBeenCalledWith("snippet_extract_variables", {
      command: "kubectl -n ${NS} apply -f ${FILE}",
    });
    expect(vars).toEqual(["FILE", "NS"]);
  });
});
