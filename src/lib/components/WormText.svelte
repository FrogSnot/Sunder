<!--  This is just me having fun hehe ;)  -->
<script lang="ts">
  let { text }: { text: string } = $props();
</script>

<span class="worm-text">
  {#each text.split("") as char, i}
    <span style="--i: {i}">{char === " " ? "\u00a0" : char}</span>
  {/each}
</span>

<style>
  .worm-text span {
    display: inline-block;
    animation: worm 3s ease-in-out infinite;
    animation-delay: calc(var(--i) * 60ms);
  }

  @keyframes worm {
    0%, 10%, 100% { transform: translateY(0); }
    5% { transform: translateY(-3px); }
  }
</style>
