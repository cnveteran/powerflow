<script setup lang="tsx">
import { Battery, CloudLightningIcon, Cpu, Laptop, Monitor, Smartphone } from 'lucide-vue-next'
import CommonTooltip from './CommonTooltip.vue'

const formatter = new Intl.NumberFormat('en-US', {
  maximumFractionDigits: 1,
  minimumFractionDigits: 1,
})

interface FlowItemProps {
  tooltip: string
  icon: Component
  color: string
}

const colorMap = {
  'text-orange-500': 'text-orange-950 dark:text-orange-50 hover:bg-orange-500/5 hover:border-orange-500/20',
  'text-primary': 'text-primary hover:bg-primary/5 hover:border-primary/20',
  'text-sky-400': 'text-sky-950 dark:text-sky-50 hover:bg-sky-400/5 hover:border-sky-400/20',
  'text-violet-500': 'text-violet-950 dark:text-violet-50 hover:bg-violet-500/5 hover:border-violet-500/20',
}

const FlowItem: Component = ({ tooltip, icon, color }: FlowItemProps, { slots }) => {
  return (
    <CommonTooltip content={tooltip} as-child>
      <div
        class={`
        w-24 shrink-0 flex justify-center items-center gap-2
        rounded-lg border bg-background px-2 py-1.5 cursor-pointer
        transition-colors ${colorMap[color]}`}
      >
        { h(icon, { class: `h-4 w-4 ${color}` }) }
        <span class="text-xs font-medium">
          { slots.default?.() }
          <span class="ml-[1px]">w</span>
        </span>
      </div>
    </CommonTooltip>
  )
}
const power = usePower()
</script>

<template>
  <Card class="flex-1">
    <CardHeader>
      <CardTitle>{{ $t('power_flow') }}</CardTitle>
    </CardHeader>
    <CardContent>
      <Skeleton v-if="power.isLoading" class="w-full h-[120px]" />
      <div
        v-else
        class="flex justify-between items-center w-full rounded-lg border bg-muted/50 p-4 font-mono text-secondary-foreground text-xs h-[120px]"
        :class="[power.isCharging ? '' : 'flex-row-reverse']"
      >
        <FlowItem
          v-if="power.isCharging"
          :tooltip="$t('flow.adapter_power')"
          :icon="CloudLightningIcon"
          color="text-orange-500"
        >
          {{ formatter.format(power.systemIn + power.efficiencyLoss) }}
        </FlowItem>

        <CommonTooltip
          v-if="power.isCharging"
          :content="`${$t('flow.power_loss')}: ${formatter.format(power.efficiencyLoss)}W`"
          as-child
        >
          <Shimmer
            :repeat-delay="1500"
            class="rounded-full mx-2 w-full
          [--base-color:#007aff]
          [--base-gradient-color:#5ac8fa]
          dark:[--base-color:#0a84ff]
          dark:[--base-gradient-color:#409cff]"
          >
            <div class="h-1 cursor-pointer" />
          </Shimmer>
        </CommonTooltip>

        <div class="flex flex-col items-center gap-2 bg-muted/50 rounded-lg border p-2">
          <div v-if="!power.isRemote" class="flex gap-4" color="text-primary">
            <FlowItem :tooltip="$t('flow.screen_power')" :icon="Monitor" color="text-primary">
              {{ formatter.format(power.brightnessPower || 0) }}
            </FlowItem>

            <FlowItem :tooltip="$t('flow.heatpipe_power')" :icon="Cpu" color="text-violet-500">
              {{ formatter.format(power.heatpipePower || 0) }}
            </FlowItem>
          </div>

          <FlowItem
            :tooltip="$t('flow.system_total')"
            :icon="power.isRemote ? Smartphone : Laptop"
            color="text-sky-400"
          >
            {{ formatter.format(power.systemLoad) }}
          </FlowItem>
        </div>

        <Shimmer
          :delay="2000"
          :repeat-delay="1500"
          class="rounded-full mx-2 w-full
          [--base-color:#007aff]
          [--base-gradient-color:#5ac8fa]
          dark:[--base-color:#0a84ff]
          dark:[--base-gradient-color:#409cff]"
        >
          <div class="h-1 cursor-pointer" />
        </Shimmer>

        <FlowItem :tooltip="power.isCharging ? $t('flow.battery_in') : $t('flow.battery_out')" :icon="Battery" color="text-primary">
          {{ formatter.format(power.batteryPower) }}
        </FlowItem>
      </div>
    </CardContent>
  </Card>
</template>
