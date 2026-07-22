<template>
  <main class="mx-auto flex w-full max-w-7xl flex-col gap-4 px-4 pb-6 pt-3 sm:px-6">
    <section class="grid gap-4 lg:grid-cols-[minmax(280px,0.85fr)_minmax(440px,1.6fr)]">
      <PowerStatus />
      <PowerFlow />
    </section>
    <PowerUsageChart />
    <TechnicalDetail />
  </main>
</template>
