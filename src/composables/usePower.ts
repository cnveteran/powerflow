import type { InterfaceType, NormalizedResource } from '@/bindings'
import type { Reactive } from 'vue'
import { events } from '@/bindings'
import { selectDeviceValue } from '@/lib/power'

import { computed, reactive } from 'vue'
import { useTab } from './useTab'

const MAX_STATISTICS_LENGTH = 20
const LOCAL_UPDATE_INTERVAL = 3

export interface StatisticData {
  'time': string
  'System Power': number
  'System In': number
  'Battery Level': number
  'Screen Power'?: number
  'Heatpipe Power'?: number
}

interface RawPowerData {
  data: NormalizedResource
  statistics: StatisticData[]
}

function trimStatistics(statistics: StatisticData[]) {
  if (statistics.length > MAX_STATISTICS_LENGTH)
    statistics.shift()
}

const localPowerData: Reactive<RawPowerData> = reactive({
  data: {} as NormalizedResource,
  statistics: [],
})

let localUpdateCount = 0

events.powerTickEvent.listen(async ({ payload: { data } }) => {
  localPowerData.data = data

  localUpdateCount++
  if (localUpdateCount < LOCAL_UPDATE_INTERVAL)
    return
  localUpdateCount = 0

  trimStatistics(localPowerData.statistics)

  localPowerData.statistics.push({
    'time': new Date().toLocaleTimeString(),
    'System Power': data.systemLoad,
    'System In': data.systemIn,
    'Battery Level': data.batteryLevel,
    'Screen Power': data.brightnessPower,
    'Heatpipe Power': data.heatpipePower,
  })
})

events.devicePowerTickEvent.listen(({ payload: { data, udid } }) => {
  const deviceData = getOrCreateDeviceData(udid)
  deviceData.data = data

  const statistics = deviceData.statistics
  trimStatistics(statistics)

  const time = new Date(data.lastUpdate * 1000).toLocaleTimeString()

  if (!statistics.length || time !== statistics[statistics.length - 1]?.time) {
    statistics.push({
      time,
      'System Power': data.systemLoad,
      'System In': data.systemIn,
      'Battery Level': data.batteryLevel,
    })
  }
})

export type RemotePowerData = RawPowerData & {
  name: string
  offline: boolean
  interface: Set<InterfaceType>
}

interface PowerData {
  local: RawPowerData
  remote: Record<string, RemotePowerData>
}

const power = reactive<PowerData>({
  local: localPowerData,
  remote: {},
})

const tab = useTab()
const removalTimers = new Map<string, number>()

function getOrCreateDeviceData(udid: string): RemotePowerData {
  if (!power.remote[udid]) {
    power.remote[udid] = {
      data: {} as NormalizedResource,
      statistics: [],
      name: '',
      offline: false,
      interface: new Set(),
    }
  }
  return power.remote[udid]
}

events.deviceEvent.listen(({ payload }) => {
  const deviceData = getOrCreateDeviceData(payload.udid)

  if (payload.action === 'Attached') {
    const timer = removalTimers.get(payload.udid)
    if (timer !== undefined) {
      window.clearTimeout(timer)
      removalTimers.delete(payload.udid)
    }
    deviceData.interface.add(payload.interface)
    deviceData.offline = false
  }
  else if (payload.action === 'Detached') {
    deviceData.interface.delete(payload.interface)
  }
  if (deviceData.interface.size === 0) {
    deviceData.offline = true
    const existingTimer = removalTimers.get(payload.udid)
    if (existingTimer !== undefined)
      window.clearTimeout(existingTimer)
    const timer = window.setTimeout(() => {
      if (power.remote[payload.udid]?.offline) {
        delete power.remote[payload.udid]
        if (tab.value === payload.udid)
          tab.value = 'local'
      }
      removalTimers.delete(payload.udid)
    }, 30_000)
    removalTimers.set(payload.udid, timer)
  }
})

const emptyPower = {} as NormalizedResource

const currentPower = computed<RawPowerData>(() => {
  return selectDeviceValue<RawPowerData>(
    power.local,
    power.remote,
    tab.value,
    { data: emptyPower, statistics: [] },
  )
})

export function usePower() {
  return computed(() => {
    const data = currentPower.value.data ?? emptyPower
    const hasData = data != null && Object.keys(data).length > 0
    return {
      ...data,
      // Keep last known values when the window is hidden — don't flash skeletons.
      isLoading: !hasData,
      isRemote: tab.value !== 'local',
      statistics: currentPower.value.statistics ?? [],
    }
  })
}

export function usePowerData() {
  return power
}

export function usePowerRaw() {
  return computed<
    RawPowerData & { isLocal: true } |
    RemotePowerData & { isLocal: false }
  >(() => {
    const isLocal = tab.value === 'local'
    if (isLocal) {
      return {
        ...power.local,
        isLocal: true as const,
      }
    }
    return {
      ...(power.remote[tab.value] ?? {
        data: emptyPower,
        statistics: [],
        name: tab.value,
        offline: true,
        interface: new Set<InterfaceType>(),
      }),
      isLocal: false as const,
    }
  })
}
