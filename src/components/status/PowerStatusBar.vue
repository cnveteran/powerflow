<script setup lang="ts">
import type { UnwrapRef } from 'vue'
import { useI18n } from 'vue-i18n'

const power = usePower()
const { t } = useI18n()

const colors: Record<string, string> = {
  screen: 'bg-power-screen',
  heatpipe: 'bg-power-thermal',
  systemOther: 'bg-power-system',
  batteryIn: 'bg-power-battery',
  systemTotal: 'bg-power-system',
  powerLoss: 'bg-power-loss',
}

const localeMap = computed(() => ({
  screen: t('status.screen_power'),
  heatpipe: t('status.heatpipe_power'),
  systemOther: t('status.system_other'),
  batteryIn: t('status.battery_in'),
  systemTotal: t('status.system_total'),
  powerLoss: t('status.power_loss'),
}))

interface BarData {
  parts: [string, {
    value: number
    left: number
    color: string
    locale: string
  }][]
  sum: number
}

const data = ref<BarData>()
const handle = watchEffect(() => {
  const parts: Record<string, number> = {
    systemTotal: power.value.systemLoad,
  }

  if (power.value.isCharging) {
    parts.batteryIn = power.value.systemIn - parts.systemTotal
    parts.powerLoss = power.value.efficiencyLoss
  }

  if (!power.value.isRemote) {
    parts.screen = power.value.brightnessPower
    parts.heatpipe = power.value.heatpipePower
    parts.systemOther = Math.max(0, parts.systemTotal - parts.screen - parts.heatpipe)
    delete parts.systemTotal
  }

  let current = 0
  const sorted = Object.entries(parts)
    .filter(([, value]) => Number.isFinite(value) && value > 0)
    .sort((a, b) => b[1] - a[1])
    .map(([key, value]) => {
      const ret = [key, current] as const
      current += value
      return ret
    })

  const sum = power.value.isCharging
    ? power.value.systemIn + power.value.efficiencyLoss
    : power.value.systemLoad
  const safeSum = Number.isFinite(sum) && sum > 0 ? sum : 1
  data.value = {
    parts: sorted
      .map(([key, left]) =>
        [key, {
          value: parts[key],
          left: left / safeSum,
          color: colors[key] || 'bg-power-system',
          locale: localeMap.value[key as keyof UnwrapRef<typeof localeMap>],
        }],
      ),
    sum: safeSum,
  }
})

const hovered = ref<string | null>(null)

watchEffect(() => {
  hovered.value ? handle.pause() : handle.resume()
})
</script>

<template>
  <!-- Charging: total = powerloss + screen + heatpipe + system other + battery in -->
  <!-- Not Charging: total = screen + heatpipe + system other -->
  <div
    v-if="data"
    class="relative flex h-3 overflow-hidden rounded bg-muted transition-all duration-300"
    @mouseleave="hovered = null"
  >
    <CommonTooltip
      v-for="[key, { value, left, color, locale }] in data.parts"
      :key="key"
      as-child
    >
      <template #popper>
        <span>{{ locale }}</span>
        <span class="ml-1 font-mono font-extrabold">{{ value.toFixed(1) }} W</span>
      </template>
      <button
        type="button"
        class="absolute bottom-0 top-0 transition-all duration-300 focus-visible:z-10 focus-visible:ring-2 focus-visible:ring-ring"
        :class="[hovered && hovered !== key ? 'opacity-20' : 'opacity-100', color]"
        :style="{ width: `calc(${(value / data.sum) * 100}% + 1px)`, left: `${left * 100}%` }"
        :aria-label="`${locale}: ${value.toFixed(1)} W`"
        @mouseover="hovered = key"
        @focus="hovered = key"
        @blur="hovered = null"
      />
    </CommonTooltip>
  </div>
</template>
