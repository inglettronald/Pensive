// The recorder only gets a frame when the page paints, and GPUI's canvas doesn't paint while
// nothing changes, so a pause would vanish from the video. This changes one corner pixel by an
// invisible amount every frame, so the page paints all the time.
(() => {
    if (document.getElementById("keep-painting")) return;
    const pixel = document.createElement("div");
    pixel.id = "keep-painting";
    pixel.style.cssText =
        "position:fixed;right:0;bottom:0;width:1px;height:1px;pointer-events:none";
    document.body.appendChild(pixel);
    let odd = false;
    const paint = () => {
        odd = !odd;
        pixel.style.background = odd ? "rgba(0,0,0,0.01)" : "rgba(0,0,0,0.02)";
        requestAnimationFrame(paint);
    };
    paint();
})();
