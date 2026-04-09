/** A saved command snippet. */
export interface Snippet {
  id: string;
  title: string;
  description?: string;
  command: string;
  tags: string[];
  folderId?: string;
  isFavorite: boolean;
  usageCount: number;
  lastUsedAt?: string;
  createdAt: string;
  updatedAt: string;
}

/** Input for creating or updating a snippet. */
export interface SnippetInput {
  title: string;
  description?: string;
  command: string;
  tags: string[];
  folderId?: string;
  isFavorite: boolean;
}

/** A folder for organizing snippets. */
export interface SnippetFolder {
  id: string;
  name: string;
  parentId?: string;
  sortOrder: number;
  createdAt: string;
}

/** Input for creating or updating a snippet folder. */
export interface SnippetFolderInput {
  name: string;
  parentId?: string;
  sortOrder: number;
}
