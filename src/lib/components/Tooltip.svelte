<script lang="ts">
  // Sunder custom tooltip - component form of the use:tooltip action.
  // The action (src/lib/util/tooltip.svelte.ts) is the primary API for the
  // app-wide title= replacement wiring; this wrapper exists for call sites
  // that prefer the <Tooltip> element form:
  //
  //   <Tooltip text="Open playlist" placement="bottom">
  //     <button aria-label="Open playlist">...</button>
  //   </Tooltip>
  //
  // The wrapper is display:contents (no box of its own); the action is hosted
  // here and the trigger rect resolves from the first element child (see
  // resolveTriggerRect). The tooltip node itself is aria-hidden, pointer-events
  // none - the trigger's aria-label remains the real a11y surface.
  import { tooltip, type TooltipParams } from "../util/tooltip.svelte";

  let {
    text,
    placement = "top",
    delay = 450,
    children,
  }: {
    text: string;
    placement?: "top" | "bottom" | "left" | "right";
    delay?: number;
    children?: import("svelte").Snippet;
  } = $props();

  let host: HTMLElement | null = null;

  const params: TooltipParams = $derived({ text, placement, delay });

  $effect(() => {
    if (!host) return;
    // Host the action on the slotted trigger itself: display:contents elements
    // have no box, and pointerenter/focus chains through them are unreliable.
    // Children render during init, so firstElementChild exists by effect time.
    const target =
      host.firstElementChild instanceof HTMLElement ? host.firstElementChild : host;
    const api = tooltip(target, params);
    return () => api.destroy();
  });
</script>

<span bind:this={host} class="tooltip-host" style="display: contents">
  {@render children?.()}
</span>

<style>
  /* The host must never affect layout; :global for action-created nodes. */
  .tooltip-host {
    display: contents;
  }
</style>
