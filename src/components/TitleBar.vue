<script setup lang="ts">
import { commands } from '@/bindings'
import { LaptopIcon, MobileIcon } from '@radix-icons/vue'
import { ChevronLeft, History, SettingsIcon } from 'lucide-vue-next'

const tab = useTab()
const data = usePowerData()

const {
  tabName,
  tabNameLoading,
  shouldDisplayShadow,
} = useTitlebar()
</script>

<template>
  <div
    data-tauri-drag-region
    class="sticky top-0 z-10 flex h-[52px] items-center justify-between border-b border-transparent bg-background/95 px-4 pt-1 backdrop-blur transition-colors"
    :class="{ 'border-border': shouldDisplayShadow }"
  >
    <div class="ml-[4.75rem] flex min-w-0 items-center gap-2">
      <button
        v-if="$route.path !== '/'"
        type="button"
        class="rounded-md p-1 transition-colors hover:bg-muted"
        :aria-label="$t('navigation.local')"
        @click="$router.back()"
      >
        <ChevronLeft class="size-6 text-muted-foreground -translate-x-px" />
      </button>
      <div v-else class="flex items-center gap-3 font-mono text-sm">
        <TabsList>
          <TransitionGroup
            enter-from-class="w-0"
            leave-to-class="w-0"
            enter-to-class="w-[40px]"
            leave-from-class="w-[40px]"
            enter-active-class="duration-200"
            leave-active-class="duration-200"
          >
            <TabsTrigger
              key="local"
              class="px-0"
              value="local"
              :aria-label="$t('navigation.local')"
            >
              <LaptopIcon class="mx-3 size-4" :class="[tab === 'local' ? 'text-blue-500' : 'text-muted-foreground']" />
            </TabsTrigger>

            <TabsTrigger
              v-for="udid in Object.keys(data.remote)"
              :key="udid"
              class="px-0"
              :value="udid"
              :aria-label="data.remote[udid]?.name || udid"
            >
              <MobileIcon class="mx-3 size-4" :class="[tab === udid ? 'text-blue-500' : 'text-muted-foreground']" />
            </TabsTrigger>
          </TransitionGroup>
        </TabsList>
        <div class="flex flex-col -translate-y-[1px]">
          <Skeleton v-if="tabNameLoading" class="w-32 h-4" />
          <span v-else class="truncate font-semibold text-secondary-foreground">{{ tabName }}</span>
          <span class="text-[10px] leading-[10px] font-normal text-muted-foreground">
            {{ tab === 'local'
              ? $t('navigation.local')
              : Array.from(data.remote[tab]?.interface || []).join(' + ') || $t('navigation.offline') }}
          </span>
        </div>
      </div>
    </div>
    <div class="flex gap-1">
      <button
        type="button"
        class="rounded-md p-2 transition-colors hover:bg-muted"
        :aria-label="$t('navigation.history')"
        @click="$route.path.startsWith('/history') ? $router.push('/') : $router.push('/history')"
      >
        <CommonTooltip :content="$t('navigation.history')" as-child>
          <History
            :stroke-width="1.8"
            class="size-5 text-muted-foreground transition-colors"
            :class="{ 'text-foreground': $route.path.startsWith('/history') }"
          />
        </CommonTooltip>
      </button>
      <button
        type="button"
        class="rounded-md p-2 transition-colors hover:bg-muted"
        :aria-label="$t('navigation.settings')"
        @click="commands.openSettings()"
      >
        <CommonTooltip :content="$t('navigation.settings')" as-child>
          <SettingsIcon
            :stroke-width="1.8"
            class="text-muted-foreground size-5"
          />
        </CommonTooltip>
      </button>
    </div>
  </div>
</template>
