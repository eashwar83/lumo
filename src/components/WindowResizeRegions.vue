<script setup lang="ts">
import { getCurrentWindow } from "@tauri-apps/api/window";

type ResizeDirection =
    | "East"
    | "North"
    | "NorthEast"
    | "NorthWest"
    | "South"
    | "SouthEast"
    | "SouthWest"
    | "West";

const startResizeDragging = (direction: ResizeDirection, event: MouseEvent) => {
    if (event.button !== 0) return;
    void getCurrentWindow().startResizeDragging(direction);
};
</script>

<template>
    <div class="window-resize-regions" aria-hidden="true" data-window-no-drag>
        <div
            class="window-resize-region window-resize-region--north"
            data-window-no-drag
            @mousedown.stop.prevent="startResizeDragging('North', $event)"
        ></div>
        <div
            class="window-resize-region window-resize-region--south"
            data-window-no-drag
            @mousedown.stop.prevent="startResizeDragging('South', $event)"
        ></div>
        <div
            class="window-resize-region window-resize-region--east"
            data-window-no-drag
            @mousedown.stop.prevent="startResizeDragging('East', $event)"
        ></div>
        <div
            class="window-resize-region window-resize-region--west"
            data-window-no-drag
            @mousedown.stop.prevent="startResizeDragging('West', $event)"
        ></div>
        <div
            class="window-resize-region window-resize-region--north-east"
            data-window-no-drag
            @mousedown.stop.prevent="startResizeDragging('NorthEast', $event)"
        ></div>
        <div
            class="window-resize-region window-resize-region--north-west"
            data-window-no-drag
            @mousedown.stop.prevent="startResizeDragging('NorthWest', $event)"
        ></div>
        <div
            class="window-resize-region window-resize-region--south-east"
            data-window-no-drag
            @mousedown.stop.prevent="startResizeDragging('SouthEast', $event)"
        ></div>
        <div
            class="window-resize-region window-resize-region--south-west"
            data-window-no-drag
            @mousedown.stop.prevent="startResizeDragging('SouthWest', $event)"
        ></div>
    </div>
</template>

<style scoped>
.window-resize-regions {
    position: absolute;
    inset: 0;
    z-index: 10000;
    pointer-events: none;
}

.window-resize-region {
    position: absolute;
    pointer-events: auto;
}

.window-resize-region--north {
    top: 0;
    left: 10px;
    right: 10px;
    height: 6px;
    cursor: n-resize;
}

.window-resize-region--south {
    bottom: 0;
    left: 10px;
    right: 10px;
    height: 6px;
    cursor: s-resize;
}

.window-resize-region--east {
    top: 10px;
    right: 0;
    bottom: 10px;
    width: 6px;
    cursor: e-resize;
}

.window-resize-region--west {
    top: 10px;
    left: 0;
    bottom: 10px;
    width: 6px;
    cursor: w-resize;
}

.window-resize-region--north-east,
.window-resize-region--north-west,
.window-resize-region--south-east,
.window-resize-region--south-west {
    width: 10px;
    height: 10px;
}

.window-resize-region--north-east {
    top: 0;
    right: 0;
    cursor: ne-resize;
}

.window-resize-region--north-west {
    top: 0;
    left: 0;
    cursor: nw-resize;
}

.window-resize-region--south-east {
    right: 0;
    bottom: 0;
    cursor: se-resize;
}

.window-resize-region--south-west {
    bottom: 0;
    left: 0;
    cursor: sw-resize;
}
</style>
