<script setup lang="ts">
const emit = defineEmits<{
  (e: "close"): void;
}>();

const isMac = navigator.platform.toUpperCase().includes("MAC");
</script>

<template>
  <Teleport to="body">
    <div
      class="fixed inset-0 z-[9999] flex items-center justify-center"
      style="background: rgba(0,0,0,0.4)"
      @click.self="emit('close')"
    >
      <div
        class="w-[680px] max-h-[80vh] flex flex-col rounded-lg shadow-2xl"
        style="background: var(--tm-bg-elevated); border: 1px solid var(--tm-border)"
      >
        <!-- Header -->
        <div
          class="flex items-center px-3 h-10 shrink-0"
          :class="isMac ? '' : 'flex-row-reverse'"
          style="border-bottom: 1px solid var(--tm-border)"
        >
          <button
            v-if="isMac"
            class="group w-3 h-3 rounded-full bg-[#ff5f57] hover:brightness-90 transition
                   flex items-center justify-center mr-3 shrink-0"
            @click="emit('close')"
          >
            <span class="text-[8px] leading-none text-black/60 opacity-0 group-hover:opacity-100">&#x2715;</span>
          </button>
          <span class="text-sm font-semibold flex-1" style="color: var(--tm-text-primary)">
            Privacy Policy
          </span>
          <button
            v-if="!isMac"
            class="tm-icon-btn p-1 rounded"
            @click="emit('close')"
          >
            <span class="text-sm">&#x2715;</span>
          </button>
        </div>

        <!-- Body -->
        <div class="flex-1 overflow-y-auto px-6 py-5 text-sm leading-relaxed" style="color: var(--tm-text-secondary)">
          <p class="mb-4 text-xs" style="color: var(--tm-text-muted)">Last updated: 2026-03-28</p>

          <h3 class="font-semibold mb-2" style="color: var(--tm-text-primary)">Overview</h3>
          <p class="mb-4">
            Termex is an open-source, local-first SSH client. Your privacy is a core design principle
            — Termex does not collect, transmit, or store any user data on external servers.
          </p>

          <h3 class="font-semibold mb-2" style="color: var(--tm-text-primary)">Data Storage</h3>
          <p class="mb-2">All data is stored locally on your device:</p>
          <ul class="list-disc pl-5 mb-4 space-y-1">
            <li><strong>Server configurations</strong> — saved in a locally encrypted SQLite database (termex.db)</li>
            <li><strong>Credentials</strong> (SSH passwords, passphrases, AI API keys) — stored in the OS credential manager (macOS Keychain / Windows Credential Manager / Linux Secret Service); termex.db only stores keychain reference IDs, never actual credentials</li>
            <li><strong>Settings and preferences</strong> — stored locally in termex.db</li>
          </ul>

          <h3 class="font-semibold mb-2" style="color: var(--tm-text-primary)">Network Communication</h3>
          <p class="mb-2">Termex only initiates network connections in the following cases:</p>
          <ol class="list-decimal pl-5 mb-4 space-y-1">
            <li><strong>SSH connections</strong> — to servers you explicitly configure and connect to</li>
            <li><strong>SFTP transfers</strong> — file operations you explicitly initiate</li>
            <li><strong>AI provider API calls</strong> — sent directly to the AI provider you configure (e.g., OpenAI, Claude, Ollama), using your own API key; Termex does not proxy or intercept these requests</li>
            <li><strong>Update checks</strong> — optional checks to GitHub Releases API to detect new versions</li>
          </ol>

          <h3 class="font-semibold mb-2" style="color: var(--tm-text-primary)">What Termex Does NOT Do</h3>
          <ul class="list-disc pl-5 mb-4 space-y-1">
            <li>Does not collect telemetry or analytics</li>
            <li>Does not send crash reports</li>
            <li>Does not track usage behavior</li>
            <li>Does not transmit credentials to any third party</li>
            <li>Does not include ads or third-party tracking SDKs</li>
            <li>Does not require account registration</li>
          </ul>

          <h3 class="font-semibold mb-2" style="color: var(--tm-text-primary)">AI Features</h3>
          <ul class="list-disc pl-5 mb-4 space-y-1">
            <li>Commands and context are sent directly to your configured AI provider</li>
            <li>Termex never acts as a middleware or proxy</li>
            <li>No data passes through Termex servers (there are none)</li>
            <li>You can use local AI models (e.g., Ollama) for fully offline operation</li>
          </ul>

          <h3 class="font-semibold mb-2" style="color: var(--tm-text-primary)">Open Source</h3>
          <p>
            Termex is fully open source under the MIT license. You can audit the source code at any time.
          </p>
        </div>
      </div>
    </div>
  </Teleport>
</template>
