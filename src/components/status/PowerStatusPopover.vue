<script setup lang="ts">
import { commands } from '@/bindings'

provide('isPopover', true)
</script>

<template>
  <div class="w-full rounded-xl border bg-background p-2 shadow-lg">
    <slot is-popover />
    <button
      type="button"
      class="mt-1 w-full rounded-md border bg-muted/50 px-3 py-2 text-sm font-medium transition-colors hover:bg-muted"
      @click="commands.openApp()"
    >
      {{ $t('popover.open_app') }}
    </button>
  </div>
</template>
