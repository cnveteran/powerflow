<script setup lang="ts">
import type { BulletLegendItemInterface } from '@unovis/ts'
import { omit } from '@unovis/ts'
import { VisTooltip } from '@unovis/vue'
import { type Component, createApp } from 'vue'
import { ChartTooltip } from '.'

const props = withDefaults(defineProps<{
  selector: string
  index: string
  items?: BulletLegendItemInterface[]
  valueFormatter?: (tick: number, i?: number, ticks?: number[]) => string
  customTooltip?: Component
}>(), {
  valueFormatter: (tick: number) => `${tick}`,
})

// Use weakmap to store reference to each datapoint for Tooltip
const wm = new WeakMap()
function template(d: any, i: number, elements: (HTMLElement | SVGElement)[]) {
  if (props.index in d) {
    if (wm.has(d)) {
      return wm.get(d)
    }
    else {
      const componentDiv = document.createElement('div')
      const omittedData = Object.entries(omit(d, [props.index])).map(([key, value]) => {
        const legendReference = props.items?.find(i => i.name === key)
        return { ...legendReference, value: props.valueFormatter(value) }
      })
      const TooltipComponent = props.customTooltip ?? ChartTooltip
      const app = createApp(TooltipComponent, { title: d[props.index], data: omittedData })
      app.mount(componentDiv)
      const html = componentDiv.innerHTML
      app.unmount()
      wm.set(d, html)
      return html
    }
  }

  else {
    const data = d.data

    if (wm.has(data)) {
      return wm.get(data)
    }
    else {
      const style = getComputedStyle(elements[i])
      const omittedData = [{ name: data.name, value: props.valueFormatter(data[props.index]), color: style.fill }]
      const componentDiv = document.createElement('div')
      const TooltipComponent = props.customTooltip ?? ChartTooltip
      const app = createApp(TooltipComponent, { title: data[props.index], data: omittedData })
      app.mount(componentDiv)
      const html = componentDiv.innerHTML
      app.unmount()
      wm.set(data, html)
      return html
    }
  }
}
</script>

<template>
  <VisTooltip
    :horizontal-shift="20" :vertical-shift="20" :triggers="{
      [selector]: template,
    }"
  />
</template>
