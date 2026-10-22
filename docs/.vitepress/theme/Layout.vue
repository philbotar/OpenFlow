<script setup lang="ts">
import DefaultTheme from 'vitepress/theme-without-fonts'
import { withBase } from 'vitepress'
import { onMounted, ref } from 'vue'

const { Layout } = DefaultTheme
const video = ref<HTMLVideoElement | null>(null)

onMounted(() => {
  // Keep the poster frame for people who ask for reduced motion.
  if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) video.value?.pause()
})
</script>

<template>
  <Layout>
    <template #home-hero-image>
      <figure class="hero-demo">
        <video
          ref="video"
          :src="withBase('/media/workflow-demo.mp4')"
          :poster="withBase('/media/workflow-demo-poster.png')"
          autoplay
          muted
          loop
          playsinline
          preload="metadata"
          aria-label="OpenFlow running a workflow: one agent finishes, then two run in parallel before a final brief."
        />
      </figure>
    </template>
  </Layout>
</template>
