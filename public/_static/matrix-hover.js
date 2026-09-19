/**
 * Matrix-style pointer-trail effect.
 *
 * Framework-free implementation with plain DOM APIs so it can run on any
 * page (no React, no build step).
 *
 * Usage:
 *   const destroy = initMatrixHoverEffect(container);
 *   // container must be position: relative (or similar) and sized;
 *   // the canvas is absolutely positioned to fill it.
 */
function initMatrixHoverEffect(container, options = {}) {
  const CHARS = '/ > < | _ : . 0 1 { }';
  const CHAR_COUNT = CHARS.length;
  const GRID_SPACING = options.gridSpacing ?? 15;
  const FONT_SIZE = options.fontSize ?? 12;
  const FONT_FAMILY =
    options.fontFamily ??
    "'Apercu Mono Pro', 'JetBrains Mono', ui-monospace, monospace";
  const INFLUENCE_RADIUS = options.influenceRadius ?? 140;
  const TRAIL_LIFETIME = options.trailLifetime ?? 70; // frames
  const MAX_TRAIL_POINTS = options.maxTrailPoints ?? 60;
  const MIN_MOVE_DIST_SQ = options.minMoveDistSq ?? 40;
  const MAX_ALPHA = options.maxAlpha ?? 0.55;
  const COLOR = options.color ?? '#e5490a';
  const MASK_IMAGE =
    options.maskImage ??
    'radial-gradient(ellipse 64% 70% at 50% 50%, black 30%, rgba(0,0,0,0.55) 62%, transparent 92%)';

  const canvas = document.createElement('canvas');
  canvas.setAttribute('aria-hidden', 'true');
  canvas.className = options.className ?? 'matrix-hover-canvas';
  canvas.style.maskImage = MASK_IMAGE;
  canvas.style.webkitMaskImage = MASK_IMAGE;
  container.appendChild(canvas);

  const ctx = canvas.getContext('2d');
  if (!ctx) return () => canvas.remove();

  const reducedMotion =
    window.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false;
  const dpr = Math.max(1, window.devicePixelRatio || 1);

  let width = 0;
  let height = 0;
  let cols = 0;
  let cellCount = 0;
  let cellX = new Float32Array();
  let cellY = new Float32Array();
  let cellGlyph = new Uint8Array();

  const resize = () => {
    const rect = container.getBoundingClientRect();
    width = Math.round(rect.width);
    height = Math.round(rect.height);
    if (width === 0 || height === 0) return;

    canvas.width = width * dpr;
    canvas.height = height * dpr;
    canvas.style.width = `${width}px`;
    canvas.style.height = `${height}px`;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

    cols = Math.ceil(width / GRID_SPACING) + 1;
    const rows = Math.ceil(height / GRID_SPACING) + 1;
    cellCount = cols * rows;
    cellX = new Float32Array(cellCount);
    cellY = new Float32Array(cellCount);
    cellGlyph = new Uint8Array(cellCount);

    for (let row = 0; row < rows; row++) {
      for (let col = 0; col < cols; col++) {
        const i = row * cols + col;
        cellX[i] = col * GRID_SPACING + GRID_SPACING * 0.5;
        cellY[i] = row * GRID_SPACING + GRID_SPACING * 0.5;
        cellGlyph[i] = (Math.random() * CHAR_COUNT) | 0;
      }
    }
  };

  const trail = [];
  let lastX = -9000;
  let lastY = -9000;
  let frame = 0;
  let rafId = 0;
  let running = false;

  const draw = () => {
    if (width === 0 || height === 0) return;
    ctx.clearRect(0, 0, width, height);
    ctx.font = `${FONT_SIZE}px ${FONT_FAMILY}`;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';

    const radiusSq = INFLUENCE_RADIUS * INFLUENCE_RADIUS;
    const n = trail.length;

    // Bounding box of all live trail points, to skip grid cells far from any of them.
    let minX = Infinity;
    let minY = Infinity;
    let maxX = -Infinity;
    let maxY = -Infinity;
    for (let i = 0; i < n; i++) {
      const p = trail[i];
      if (frame - p.birth > TRAIL_LIFETIME) continue;
      if (p.x < minX) minX = p.x;
      if (p.x > maxX) maxX = p.x;
      if (p.y < minY) minY = p.y;
      if (p.y > maxY) maxY = p.y;
    }
    minX -= INFLUENCE_RADIUS;
    minY -= INFLUENCE_RADIUS;
    maxX += INFLUENCE_RADIUS;
    maxY += INFLUENCE_RADIUS;

    for (let c = 0; c < cellCount; c++) {
      const x = cellX[c];
      const y = cellY[c];
      if (x < minX || x > maxX || y < minY || y > maxY) continue;

      // Intensity = strongest (nearest + freshest) influence from any trail point.
      let intensity = 0;
      for (let i = 0; i < n; i++) {
        const p = trail[i];
        const age = frame - p.birth;
        if (age > TRAIL_LIFETIME) continue;
        const dx = x - p.x;
        const dy = y - p.y;
        const distSq = dx * dx + dy * dy;
        if (distSq > radiusSq) continue;
        const distNorm = Math.sqrt(distSq) / INFLUENCE_RADIUS;
        const ageNorm = age / TRAIL_LIFETIME;
        const value = (1 - distNorm) ** (1 + ageNorm * 5) * (1 - ageNorm * ageNorm);
        if (value > intensity) intensity = value;
      }
      if (intensity < 0.03) continue;

      let glyph;
      if (intensity > 0.25) {
        const shift = ((frame * 0.06 + intensity * 2) | 0) & 32767;
        glyph = CHARS[(cellGlyph[c] + shift) % CHAR_COUNT];
      } else {
        glyph = CHARS[cellGlyph[c]];
      }
      if (glyph === ' ') continue;

      const alpha = intensity > 0.25 ? intensity * MAX_ALPHA : intensity * 0.4;
      if (alpha < 0.01) continue;

      ctx.globalAlpha = alpha;
      ctx.fillStyle = COLOR;
      ctx.fillText(glyph, x, y);
    }
    ctx.globalAlpha = 1;
  };

  const tick = () => {
    frame++;
    while (trail.length > 0 && frame - trail[0].birth > TRAIL_LIFETIME) trail.shift();
    draw();
    if (trail.length > 0) {
      rafId = requestAnimationFrame(tick);
    } else {
      running = false;
      draw();
    }
  };

  const ensureRunning = () => {
    if (!running) {
      running = true;
      rafId = requestAnimationFrame(tick);
    }
  };

  const onPointerMove = (e) => {
    const rect = container.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const dx = x - lastX;
    const dy = y - lastY;
    if (dx * dx + dy * dy > MIN_MOVE_DIST_SQ) {
      trail.push({ x, y, birth: frame });
      if (trail.length > MAX_TRAIL_POINTS) trail.shift();
      lastX = x;
      lastY = y;
      ensureRunning();
    }
  };

  const onPointerLeave = () => {
    lastX = -9000;
    lastY = -9000;
  };

  resize();
  if (!reducedMotion) {
    container.addEventListener('pointermove', onPointerMove);
    container.addEventListener('pointerleave', onPointerLeave);
  }

  const resizeObserver = new ResizeObserver(() => {
    resize();
    if (!running) draw();
  });
  resizeObserver.observe(container);

  return function destroy() {
    resizeObserver.disconnect();
    cancelAnimationFrame(rafId);
    container.removeEventListener('pointermove', onPointerMove);
    container.removeEventListener('pointerleave', onPointerLeave);
    canvas.remove();
  };
}

/**
 * Starts the trail on the page body. The colour is read from the
 * stylesheet rather than repeated here, so the trail and the bars cannot
 * drift apart.
 */
(function () {
  var green = getComputedStyle(document.documentElement)
    .getPropertyValue('--up')
    .trim();

  initMatrixHoverEffect(document.body, {
    color: green,
    // No vignette: the page scrolls, and a mask centred on the document
    // would fade the trail out wherever the reader happens to be.
    maskImage: 'none'
  });
})();
