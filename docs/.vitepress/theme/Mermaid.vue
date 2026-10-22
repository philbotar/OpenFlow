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
      theme: 'base',
      themeVariables: dark
        ? {
            background: '#101010',
            primaryColor: '#202021',
            primaryTextColor: '#f2f2f3',
            primaryBorderColor: '#85858b',
            secondaryColor: '#181818',
            tertiaryColor: '#181818',
            lineColor: '#b0b0b3',
            textColor: '#f2f2f3',
            edgeLabelBackground: '#101010',
          }
        : {
            background: '#fcfbf8',
            primaryColor: '#f6f3ed',
            primaryTextColor: '#18181b',
            primaryBorderColor: '#8a877f',
            secondaryColor: '#f6f3ed',
            tertiaryColor: '#fcfbf8',
            lineColor: '#66645d',
            textColor: '#18181b',
            edgeLabelBackground: '#fcfbf8',
          },
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
