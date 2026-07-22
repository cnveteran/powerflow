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
  color: 'text-power-input' | 'text-power-system' | 'text-power-battery' | 'text-power-screen' | 'text-power-thermal'
}

const colorMap = {
  'text-power-input': 'hover:bg-power-input/5 hover:border-power-input/20',
  'text-power-system': 'hover:bg-power-system/5 hover:border-power-system/20',
  'text-power-battery': 'hover:bg-power-battery/5 hover:border-power-battery/20',
  'text-power-screen': 'hover:bg-power-screen/5 hover:border-power-screen/20',
  'text-power-thermal': 'hover:bg-power-thermal/5 hover:border-power-thermal/20',
}

const FlowItem: Component = ({ tooltip, icon, color }: FlowItemProps, { slots }) => {
  return (
    <CommonTooltip content={tooltip} as-child>
      <button
        type="button"
        class={`
        w-24 shrink-0 flex justify-center items-center gap-2
        rounded-lg border bg-background px-2 py-1.5
        transition-colors ${colorMap[color]}`}
      >
        { h(icon, { class: `h-4 w-4 ${color}` }) }
        <span class="text-xs font-medium">
          { slots.default?.() }
          <span class="ml-[1px]"> W</span>
        </span>
      </button>
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
          color="text-power-input"
        >
          {{ formatter.format(power.systemIn + power.efficiencyLoss) }}
        </FlowItem>

        <CommonTooltip
          v-if="power.isCharging"
          :content="`${$t('flow.power_loss')}: ${formatter.format(power.efficiencyLoss)}W`"
          as-child
        >
          <div class="mx-2 h-1 w-full rounded-full bg-power-loss/40" />
        </CommonTooltip>

        <div class="flex flex-col items-center gap-2 bg-muted/50 rounded-lg border p-2">
          <div v-if="!power.isRemote" class="flex gap-4" color="text-blue-500">
            <FlowItem :tooltip="$t('flow.screen_power')" :icon="Monitor" color="text-power-screen">
              {{ formatter.format(power.brightnessPower || 0) }}
            </FlowItem>

            <FlowItem :tooltip="$t('flow.heatpipe_power')" :icon="Cpu" color="text-power-thermal">
              {{ formatter.format(power.heatpipePower || 0) }}
            </FlowItem>
          </div>

          <FlowItem
            :tooltip="$t('flow.system_total')"
            :icon="power.isRemote ? Smartphone : Laptop"
            color="text-power-system"
          >
            {{ formatter.format(power.systemLoad) }}
          </FlowItem>
        </div>

        <div class="mx-2 h-1 w-full rounded-full bg-power-battery/50" />

        <FlowItem :tooltip="power.isCharging ? $t('flow.battery_in') : $t('flow.battery_out')" :icon="Battery" color="text-power-battery">
          {{ formatter.format(power.batteryPower) }}
        </FlowItem>
      </div>
    </CardContent>
  </Card>
</template>
