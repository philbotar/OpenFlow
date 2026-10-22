<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'

const props = defineProps<{ code: string }>()
const source = decodeURIComponent(props.code)
const svg = ref('')
const failed = ref(false)

let counter = 0
let observer: MutationObserver | undefined
let disposed = false

async function draw() {
  try {
    const [{ default: mermaid }] = await Promise.all([import('mermaid'), document.fonts.ready])
    const dark = document.documentElement.classList.contains('dark')
    mermaid.initialize({
      startOnLoad: false,
      securityLevel: 'strict',
      theme: dark ? 'dark' : 'neutral',
      fontFamily: getComputedStyle(document.body).fontFamily,
    })
    const id = `mermaid-${Math.random().toString(36).slice(2)}-${counter++}`
    const { svg: rendered } = await mermaid.render(id, source)
    if (!disposed) {
      svg.value = rendered
      failed.value = false
    }
  } catch {
    if (!disposed) failed.value = true
  }
}

onMounted(() => {
  draw()
  observer = new MutationObserver(draw)
  observer.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] })
})

onBeforeUnmount(() => {
  disposed = true
  observer?.disconnect()
})
</script>

<template>
  <div class="mermaid-diagram">
    <div v-if="svg && !failed" class="mermaid-svg" v-html="svg" />
    <pre v-else class="mermaid-source"><code>{{ source }}</code></pre>
  </div>
</template>
