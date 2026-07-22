<script setup lang="ts">
import type { ChargingHistory } from '@/bindings'
import { events } from '@/bindings'
import { useHistory } from '@/composables/useHistory'
import { format } from 'date-fns'
import { Info } from 'lucide-vue-next'

const { selectedItem, history: { data, isLoading, update } } = useHistory()
const deviceFilter = ref('all')
const dateFilter = ref('')

const devices = computed(() =>
  Array.from(
    new Map(
      (data.value ?? []).map(item => [item.udid || 'local', item.name || 'Mac']),
    ),
  ),
)

const filteredHistory = computed(() =>
  (data.value ?? []).filter((item) => {
    const matchesDevice = deviceFilter.value === 'all'
      || (item.udid || 'local') === deviceFilter.value
    const matchesDate = !dateFilter.value
      || format(new Date(item.timestamp * 1000), 'yyyy-MM-dd') === dateFilter.value
    return matchesDevice && matchesDate
  }),
)

onMounted(() => {
  const unlisten = events.historyRecordedEvent.listen(() => {
    update()
  })

  onScopeDispose(() => unlisten.then(f => f()))
})
</script>

<template>
  <div
    v-if="!isLoading && !data?.length"
    class="flex h-full w-full flex-col items-center justify-center gap-2 text-muted-foreground"
  >
    <Info class="w-6 h-6" />
    <span>{{ $t('history.empty') }}</span>
    <span class="mb-16 text-sm">{{ $t('history.empty_desc') }}</span>
  </div>
  <div v-else class="grid h-[calc(100vh-52px)] grid-cols-[minmax(280px,360px)_1fr]">
    <aside class="flex min-w-0 flex-col gap-3 border-r px-4 pb-4">
      <h1 class="pt-1 text-lg font-semibold">
        {{ $t('history.title') }}
      </h1>
      <div class="grid grid-cols-2 gap-2">
        <select v-model="deviceFilter" class="h-8 min-w-0 rounded-md border bg-background px-2 text-xs">
          <option value="all">
            {{ $t('settings.auto') }}
          </option>
          <option v-for="[udid, name] in devices" :key="udid" :value="udid">
            {{ name }}
          </option>
        </select>
        <input v-model="dateFilter" type="date" class="h-8 min-w-0 rounded-md border bg-background px-2 text-xs">
      </div>
      <div v-if="!isLoading" class="flex h-full flex-col gap-2 overflow-y-auto pr-1">
        <HistoryListItem
          v-for="item in filteredHistory as ChargingHistory[]"
          :key="item.id"
          v-bind="item"
          class="cursor-pointer transition-colors"
          :class="{ 'bg-muted': selectedItem?.id === item.id }"
          @click="selectedItem = selectedItem?.id === item.id ? null : item"
        />
      </div>
    </aside>
    <section class="relative min-w-0 grow">
      <HistoryDetail
        v-if="selectedItem?.id"
        v-bind="selectedItem"
        class="h-full"
      />
      <div v-else class="overflow-y-auto h-full flex flex-col items-center justify-center">
        <Info class="size-6" />
        <p class="text-muted-foreground font-medium text-sm mb-10">
          {{ $t('history.select') }}
        </p>
      </div>
    </section>
  </div>
</template>
