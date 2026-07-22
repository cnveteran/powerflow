<script setup lang="ts">
import NumberFlow from '@number-flow/vue'

const power = usePower()
const { preference, isLoading } = usePreferenceAsync()
</script>

<template>
  <div>
    <Skeleton v-if="power.isLoading || isLoading" class="w-40 h-[50px] mt-2" />
    <NumberFlow
      v-else-if="preference.animationsEnabled"
      class="metric-number text-4xl font-semibold"
      :format="{ maximumFractionDigits: 1, minimumFractionDigits: 1 }"
      :value="power.isCharging ? power.systemIn : power.systemLoad"
      suffix=" W"
    />
    <div v-else class="metric-number text-4xl leading-[54px] font-semibold">
      {{ (power.isCharging ? power.systemIn : power.systemLoad).toFixed(1) }} W
    </div>
  </div>
</template>
