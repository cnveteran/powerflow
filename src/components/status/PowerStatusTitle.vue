<script setup lang="ts">
import { addSeconds, format } from 'date-fns'
import { ArrowUpDown } from 'lucide-vue-next'

const power = usePower()
const rawData = usePowerRaw()

const showRemainDuration = ref(true)
const buttonText = computed(() => {
  if (!power.value.timeRemainKnown)
    return null

  if (showRemainDuration.value) {
    const minutes = Math.max(0, Math.round(power.value.timeRemain.secs / 60))
    const hours = Math.floor(minutes / 60)

    return hours > 0 ? `${hours}h ${minutes % 60}m` : `${minutes}m`
  }
  return format(
    addSeconds(new Date(), power.value.timeRemain.secs),
    'HH:mm',
  )
})
</script>

<template>
  <div class="mr-10 flex gap-2 items-center">
    {{ power.isCharging ? $t('status.charging_power') : $t('status.system_power') }}
    <span
      v-if="power.isRemote"
      class="mr-1 size-2 rounded-full"
      :class="{
        'bg-blue-500 animate-pulse': !rawData.isLocal && !rawData.offline,
        'bg-neutral-500': !rawData.isLocal && rawData.offline,
      }"
    />
  </div>

  <Skeleton v-if="power.isLoading" class="w-24 h-6" />
  <div
    v-else-if="power.isCharging"
    class="truncate rounded-md bg-power-input/15 px-2 py-1 font-mono text-xs text-power-input"
  >
    <span class="mr-1 font-bold">{{ power.adapterWatts }}W</span>
    <span class="text-[10px] opacity-80">({{ power.adapterVoltage }}V,{{
      power.adapterAmperage }}A)</span>
  </div>
  <button
    v-else
    type="button"
    class="flex min-w-20 items-center justify-center rounded-md bg-power-system/15 px-2 py-1 font-mono text-xs text-power-system transition-colors hover:bg-power-system/20"
    :aria-pressed="!showRemainDuration"
    :aria-label="$t('status.to_empty')"
    @click.stop="showRemainDuration = !showRemainDuration"
  >
    <span class="mr-1 font-bold">{{ buttonText ?? $t('status.calculating') }}</span>
    <ArrowUpDown
      v-if="buttonText"
      class="size-3 opacity-70 transition-transform duration-200"
      :class="{ 'rotate-180': showRemainDuration }"
    />
  </button>
</template>
