// ==========================================================================
// VentaCore Studio - Hybrid Design System Runtime
// ==========================================================================

document.addEventListener('DOMContentLoaded', () => {
  // ------------------------------------------------------------------------
  // 1. Charter Rule 10: Persistent Bottom Tab Navigation
  // ------------------------------------------------------------------------
  const navTabs = document.querySelectorAll('.nav-tab');
  const pageViews = document.querySelectorAll('.page-view');

  function switchPage(pageId) {
    navTabs.forEach(t => {
      if (t.dataset.page === pageId) {
        t.classList.add('active');
      } else {
        t.classList.remove('active');
      }
    });

    pageViews.forEach(p => {
      if (p.id === `page-${pageId}`) {
        p.classList.add('active');
      } else {
        p.classList.remove('active');
      }
    });
  }

  navTabs.forEach(tab => {
    tab.addEventListener('click', () => switchPage(tab.dataset.page));
  });

  // Quick export button in header navigates to export page
  const quickExportBtn = document.getElementById('btn-quick-export');
  if (quickExportBtn) {
    quickExportBtn.addEventListener('click', () => switchPage('export'));
  }

  // ------------------------------------------------------------------------
  // 2. Cyber-Studio Rhythm & Beat Engine
  // ------------------------------------------------------------------------
  const BPM = 128.0;
  const BEAT_INTERVAL_MS = (60 / BPM) * 1000; // ~468.75 ms
  let currentBeat = 0;
  let currentBar = 12;
  let currentSubdivision = 1;

  const beatLamp = document.getElementById('beat-lamp');
  const beatDots = document.querySelectorAll('.beat-dot');
  const musicalTimeDisplay = document.getElementById('musical-time');
  const vuBarL = document.getElementById('vu-meter-l');
  const vuBarR = document.getElementById('vu-meter-r');
  const vuClipL = document.getElementById('vu-clip-l');
  const vuClipR = document.getElementById('vu-clip-r');
  const spectrumBars = document.querySelectorAll('.spectrum-bar');

  let isPlaying = false;

  function pulseBeat() {
    currentBeat = (currentBeat % 4) + 1;
    if (currentBeat === 1) {
      currentBar++;
    }

    // Downbeat dot indicators
    beatDots.forEach(dot => {
      if (parseInt(dot.dataset.beat, 10) === currentBeat) {
        dot.classList.add('active');
      } else {
        dot.classList.remove('active');
      }
    });

    // Pulse lamp: downbeat (beat 1) is intense amber; others are cyan
    if (beatLamp) {
      if (currentBeat === 1) {
        beatLamp.classList.remove('pulse');
        beatLamp.classList.add('downbeat-pulse');
        setTimeout(() => beatLamp.classList.remove('downbeat-pulse'), 140);
      } else {
        beatLamp.classList.remove('downbeat-pulse');
        beatLamp.classList.add('pulse');
        setTimeout(() => beatLamp.classList.remove('pulse'), 120);
      }
    }

    // Update musical timecode: BAR : BEAT : 1/16
    currentSubdivision = Math.floor(Math.random() * 4) + 1;
    if (musicalTimeDisplay) {
      const barStr = String(currentBar).padStart(3, '0');
      const beatStr = String(currentBeat).padStart(2, '0');
      const subStr = String(currentSubdivision).padStart(2, '0');
      musicalTimeDisplay.textContent = `${barStr} : ${beatStr} : ${subStr}`;
    }

    // Audio VU meter simulation
    if (isPlaying) {
      const peakL = 60 + Math.random() * 35;
      const peakR = 55 + Math.random() * 38;
      if (vuBarL) vuBarL.style.height = `${peakL}%`;
      if (vuBarR) vuBarR.style.height = `${peakR}%`;
      if (vuClipL) vuClipL.classList.toggle('active', peakL > 92);
      if (vuClipR) vuClipR.classList.toggle('active', peakR > 92);
    } else {
      if (vuBarL) vuBarL.style.height = '15%';
      if (vuBarR) vuBarR.style.height = '12%';
      if (vuClipL) vuClipL.classList.remove('active');
      if (vuClipR) vuClipR.classList.remove('active');
    }

    // Frequency spectrum bars jump
    spectrumBars.forEach(bar => {
      const height = isPlaying ? (20 + Math.random() * 75) : (10 + Math.random() * 20);
      bar.style.height = `${height}%`;
    });
  }

  // Run beat engine timer
  setInterval(pulseBeat, BEAT_INTERVAL_MS);

  // ------------------------------------------------------------------------
  // 3. Playback State & Transport Controls
  // ------------------------------------------------------------------------
  const playBtn = document.getElementById('btn-play-pause');
  const cutPlayBtn = document.getElementById('btn-cut-play');
  const sourceTimecode = document.getElementById('source-timecode');
  const headerTimecode = document.getElementById('header-timecode');
  const sourceScrub = document.getElementById('source-scrub');
  const songPlayhead = document.getElementById('song-playhead');

  let currentFrame = 284; // ~11s at 24fps
  const FPS = 24;

  function formatTimecode(frames) {
    const totalSeconds = Math.floor(frames / FPS);
    const ff = frames % FPS;
    const ss = totalSeconds % 60;
    const mm = Math.floor(totalSeconds / 60) % 60;
    const hh = Math.floor(totalSeconds / 3600);
    return `${String(hh).padStart(2, '0')}:${String(mm).padStart(2, '0')}:${String(ss).padStart(2, '0')}:${String(ff).padStart(2, '0')}`;
  }

  function updateTimecodeUI() {
    const tc = formatTimecode(currentFrame);
    if (sourceTimecode) sourceTimecode.textContent = tc;
    if (headerTimecode) headerTimecode.textContent = tc;
    if (sourceScrub) sourceScrub.value = currentFrame % 1000;
  }

  function togglePlayback() {
    isPlaying = !isPlaying;
    const playIcon = isPlaying ? '⏸' : '▶';
    if (playBtn) playBtn.textContent = playIcon;
    if (cutPlayBtn) cutPlayBtn.textContent = playIcon;

    if (window.__TAURI__) {
      window.__TAURI__.core.invoke('toggle_playback').catch(() => {});
    }
  }

  if (playBtn) playBtn.addEventListener('click', togglePlayback);
  if (cutPlayBtn) cutPlayBtn.addEventListener('click', togglePlayback);

  // Playhead frame ticker
  setInterval(() => {
    if (isPlaying) {
      currentFrame = (currentFrame + 1) % (FPS * 60 * 3); // 3 min loop
      updateTimecodeUI();

      // Update waveform playhead percentage
      if (songPlayhead) {
        const pct = (currentFrame % (FPS * 60)) / (FPS * 60) * 100;
        songPlayhead.style.left = `${pct}%`;
      }
    }
  }, 1000 / FPS);

  // Step Frame Buttons
  const stepBackBtn = document.getElementById('btn-step-back');
  const stepFwdBtn = document.getElementById('btn-step-fwd');
  if (stepBackBtn) {
    stepBackBtn.addEventListener('click', () => {
      if (currentFrame > 0) currentFrame--;
      updateTimecodeUI();
    });
  }
  if (stepFwdBtn) {
    stepFwdBtn.addEventListener('click', () => {
      currentFrame++;
      updateTimecodeUI();
    });
  }

  // Scrub slider listener
  if (sourceScrub) {
    sourceScrub.addEventListener('input', (e) => {
      currentFrame = parseInt(e.target.value, 10);
      updateTimecodeUI();
    });
  }

  // ------------------------------------------------------------------------
  // 4. In/Out Point Marking & Industrial NLE Controls
  // ------------------------------------------------------------------------
  let inPoint = 15;  // 15%
  let outPoint = 70; // 70%

  const inOutRange = document.getElementById('in-out-range');
  const markInBtn = document.getElementById('btn-mark-in');
  const markOutBtn = document.getElementById('btn-mark-out');
  const clearInOutBtn = document.getElementById('btn-clear-inout');
  const insertBtn = document.getElementById('btn-insert-timeline');
  const overwriteBtn = document.getElementById('btn-overwrite-timeline');

  function updateInOutDisplay() {
    if (inOutRange) {
      inOutRange.style.left = `${inPoint}%`;
      inOutRange.style.width = `${Math.max(1, outPoint - inPoint)}%`;
    }
  }

  if (markInBtn) {
    markInBtn.addEventListener('click', () => {
      const pct = (currentFrame % 1000) / 10;
      if (pct < outPoint) inPoint = pct;
      updateInOutDisplay();
    });
  }

  if (markOutBtn) {
    markOutBtn.addEventListener('click', () => {
      const pct = (currentFrame % 1000) / 10;
      if (pct > inPoint) outPoint = pct;
      updateInOutDisplay();
    });
  }

  if (clearInOutBtn) {
    clearInOutBtn.addEventListener('click', () => {
      inPoint = 0;
      outPoint = 100;
      updateInOutDisplay();
    });
  }

  if (insertBtn) {
    insertBtn.addEventListener('click', () => {
      alert(`[NLE Insert] Appended marked range (${inPoint.toFixed(0)}% -> ${outPoint.toFixed(0)}%) to Timeline Track V1.`);
    });
  }

  if (overwriteBtn) {
    overwriteBtn.addEventListener('click', () => {
      alert(`[NLE Overwrite] Overwrote timeline segment at playhead with current source clip.`);
    });
  }

  // ------------------------------------------------------------------------
  // 5. Footage Pool & Media Bin System
  // ------------------------------------------------------------------------
  const footagePool = [
    { id: 1, name: "Concert_Pyrotechnics_Close.mp4", duration: "0:42", res: "3840x2160 UHD", fps: "59.94 fps", tag: "4k", motion: "high" },
    { id: 2, name: "Drone_Neon_Cybercity_Flyover.mp4", duration: "1:18", res: "3840x2160 UHD", fps: "29.97 fps", tag: "4k", motion: "med" },
    { id: 3, name: "Lead_Guitarist_Solo_Strobe.mp4", duration: "0:28", res: "1920x1080 FHD", fps: "59.94 fps", tag: "60fps", motion: "high" },
    { id: 4, name: "Vocalist_Rain_Performance.mp4", duration: "0:54", res: "3840x2160 UHD", fps: "23.98 fps", tag: "4k", motion: "low" },
    { id: 5, name: "Crowd_Moshpit_Laser_Rays.mp4", duration: "1:05", res: "1920x1080 FHD", fps: "59.94 fps", tag: "60fps", motion: "high" },
    { id: 6, name: "Drummer_Double_Kick_Sync.mp4", duration: "0:35", res: "3840x2160 UHD", fps: "59.94 fps", tag: "4k", motion: "high" }
  ];

  const footageListElem = document.getElementById('footage-list');
  const sourceTitleElem = document.getElementById('source-title');
  const sourceResElem = document.getElementById('source-res');
  const sourceFpsElem = document.getElementById('source-fps');
  const previewActiveTitle = document.getElementById('preview-active-title');
  const inspectorClipName = document.getElementById('inspector-clip-name');

  function renderFootagePool(clips) {
    if (!footageListElem) return;
    footageListElem.innerHTML = '';

    clips.forEach(clip => {
      const item = document.createElement('div');
      item.className = 'footage-item' + (clip.id === 1 ? ' selected' : '');
      item.innerHTML = `
        <div class="footage-meta">
          <span class="footage-name">🎬 ${clip.name}</span>
          <span class="footage-info">${clip.duration} • ${clip.res} • ${clip.fps}</span>
        </div>
        <button class="btn btn-xs btn-secondary">Audition</button>
      `;

      item.addEventListener('click', () => {
        document.querySelectorAll('.footage-item').forEach(i => i.classList.remove('selected'));
        item.classList.add('selected');

        if (sourceTitleElem) sourceTitleElem.textContent = clip.name;
        if (sourceResElem) sourceResElem.textContent = clip.res;
        if (sourceFpsElem) sourceFpsElem.textContent = clip.fps;
        if (previewActiveTitle) previewActiveTitle.textContent = clip.name;
        if (inspectorClipName) inspectorClipName.textContent = `Clip: ${clip.name}`;
      });

      footageListElem.appendChild(item);
    });
  }

  renderFootagePool(footagePool);

  // Filter chips in Media Bin
  const chips = document.querySelectorAll('.chip');
  chips.forEach(chip => {
    chip.addEventListener('click', () => {
      chips.forEach(c => c.classList.remove('active'));
      chip.classList.add('active');
      const filter = chip.dataset.filter;

      if (filter === 'all') {
        renderFootagePool(footagePool);
      } else if (filter === '4k') {
        renderFootagePool(footagePool.filter(c => c.tag === '4k'));
      } else if (filter === '60fps') {
        renderFootagePool(footagePool.filter(c => c.fps.includes('59.94') || c.tag === '60fps'));
      } else if (filter === 'motion') {
        renderFootagePool(footagePool.filter(c => c.motion === 'high'));
      }
    });
  });

  // Search input in Media Bin
  const searchInput = document.getElementById('input-search-clips');
  if (searchInput) {
    searchInput.addEventListener('input', (e) => {
      const q = e.target.value.toLowerCase();
      renderFootagePool(footagePool.filter(c => c.name.toLowerCase().includes(q)));
    });
  }

  // ------------------------------------------------------------------------
  // 6. Master Song Waveform Drawing
  // ------------------------------------------------------------------------
  const waveformCanvas = document.getElementById('waveform-canvas');
  if (waveformCanvas) {
    const ctx = waveformCanvas.getContext('2d');
    const width = waveformCanvas.width;
    const height = waveformCanvas.height;

    ctx.clearRect(0, 0, width, height);

    // Draw dark background grid lines
    ctx.strokeStyle = 'rgba(255, 255, 255, 0.04)';
    ctx.lineWidth = 1;
    for (let x = 0; x < width; x += 40) {
      ctx.beginPath();
      ctx.moveTo(x, 0);
      ctx.lineTo(x, height);
      ctx.stroke();
    }

    // Draw synthetic audio energy waveform
    const grad = ctx.createLinearGradient(0, 0, 0, height);
    grad.addColorStop(0, '#00f0ff');
    grad.addColorStop(0.5, '#2563eb');
    grad.addColorStop(1, '#00f0ff');

    ctx.fillStyle = grad;
    for (let x = 0; x < width; x += 3) {
      const seed = Math.sin(x * 0.05) * Math.cos(x * 0.02);
      const barH = Math.max(4, Math.abs(seed) * (height - 8) + (Math.random() * 8));
      const y = (height - barH) / 2;
      ctx.fillRect(x, y, 2, barH);
    }
  }

  // ------------------------------------------------------------------------
  // 7. Cut Page Montage Engine Controls
  // ------------------------------------------------------------------------
  const sliderSensitivity = document.getElementById('slider-sensitivity');
  const valSensitivity = document.getElementById('val-sensitivity');
  if (sliderSensitivity && valSensitivity) {
    sliderSensitivity.addEventListener('input', (e) => {
      valSensitivity.textContent = `${e.target.value}%`;
    });
  }

  const sliderMotionBias = document.getElementById('slider-motion-bias');
  const valMotionBias = document.getElementById('val-motion-bias');
  if (sliderMotionBias && valMotionBias) {
    sliderMotionBias.addEventListener('input', (e) => {
      const v = parseInt(e.target.value, 10);
      valMotionBias.textContent = v > 70 ? 'High' : v > 35 ? 'Balanced' : 'Low';
    });
  }

  const generateMontageBtn = document.getElementById('btn-generate-montage');
  if (generateMontageBtn) {
    generateMontageBtn.addEventListener('click', () => {
      const originalText = generateMontageBtn.innerHTML;
      generateMontageBtn.innerHTML = '⚡ Analyzing Beats & Cutting...';
      generateMontageBtn.disabled = true;

      setTimeout(() => {
        generateMontageBtn.innerHTML = '✅ Beat Cut Generated (42 Slices)';
        generateMontageBtn.disabled = false;
        setTimeout(() => {
          generateMontageBtn.innerHTML = originalText;
        }, 2200);
      }, 700);
    });
  }

  // ------------------------------------------------------------------------
  // 8. Edit Page NLE Timeline Tools
  // ------------------------------------------------------------------------
  const toolButtons = document.querySelectorAll('.timeline-toolbar .btn-tool');
  toolButtons.forEach(btn => {
    btn.addEventListener('click', () => {
      toolButtons.forEach(b => b.classList.remove('active'));
      btn.classList.add('active');
    });
  });

  const speedChips = document.querySelectorAll('.speed-buttons .btn-chip');
  speedChips.forEach(chip => {
    chip.addEventListener('click', () => {
      speedChips.forEach(c => c.classList.remove('active'));
      chip.classList.add('active');
    });
  });

  // Timeline zoom slider
  const zoomSlider = document.getElementById('timeline-zoom');
  const multitrackView = document.getElementById('multitrack-view');
  if (zoomSlider && multitrackView) {
    zoomSlider.addEventListener('input', (e) => {
      const scale = e.target.value / 100;
      // Visual feedback on lane clips
      document.querySelectorAll('.timeline-clip').forEach(clip => {
        clip.style.transform = `scaleY(${Math.min(1.15, Math.max(0.85, scale))})`;
      });
    });
  }

  // ------------------------------------------------------------------------
  // 9. Export Page Preset Selector & Queue Runner
  // ------------------------------------------------------------------------
  const presetCards = document.querySelectorAll('.preset-card');
  presetCards.forEach(card => {
    card.addEventListener('click', () => {
      presetCards.forEach(c => c.classList.remove('active'));
      card.classList.add('active');
    });
  });

  const startQueueBtn = document.getElementById('btn-start-queue');
  if (startQueueBtn) {
    startQueueBtn.addEventListener('click', () => {
      const bar = document.querySelector('.job-progress-bar');
      const tag = document.querySelector('.job-tag');
      if (!bar) return;

      let pct = 0;
      startQueueBtn.disabled = true;
      startQueueBtn.textContent = 'Rendering...';

      const interval = setInterval(() => {
        pct += 5;
        bar.style.width = `${pct}%`;
        if (tag) tag.textContent = `Rendering (${pct}%)`;

        if (pct >= 100) {
          clearInterval(interval);
          startQueueBtn.disabled = false;
          startQueueBtn.textContent = 'Render Finished!';
          if (tag) tag.textContent = 'Completed (Ready)';
          setTimeout(() => {
            startQueueBtn.textContent = '⚡ Start Batch Render';
          }, 3000);
        }
      }, 100);
    });
  }

  // ------------------------------------------------------------------------
  // 10. Global Keyboard Shortcuts (Rule 10 & Pro NLE)
  // ------------------------------------------------------------------------
  window.addEventListener('keydown', (e) => {
    // Page switching shortcuts: Ctrl+1..4
    if (e.ctrlKey) {
      if (e.key === '1') { e.preventDefault(); switchPage('media'); }
      if (e.key === '2') { e.preventDefault(); switchPage('cut'); }
      if (e.key === '3') { e.preventDefault(); switchPage('edit'); }
      if (e.key === '4') { e.preventDefault(); switchPage('export'); }
    }

    // Spacebar Play/Pause
    if (e.code === 'Space' && e.target.tagName !== 'INPUT' && !e.target.isContentEditable) {
      e.preventDefault();
      togglePlayback();
    }

    // [I] Mark In / [O] Mark Out
    if (e.key === 'i' || e.key === 'I') {
      if (e.target.tagName !== 'INPUT') {
        const pct = (currentFrame % 1000) / 10;
        if (pct < outPoint) inPoint = pct;
        updateInOutDisplay();
      }
    }
    if (e.key === 'o' || e.key === 'O') {
      if (e.target.tagName !== 'INPUT') {
        const pct = (currentFrame % 1000) / 10;
        if (pct > inPoint) outPoint = pct;
        updateInOutDisplay();
      }
    }

    // Arrow keys step frame
    if (e.key === 'ArrowLeft' && e.target.tagName !== 'INPUT') {
      if (currentFrame > 0) currentFrame--;
      updateTimecodeUI();
    }
    if (e.key === 'ArrowRight' && e.target.tagName !== 'INPUT') {
      currentFrame++;
      updateTimecodeUI();
    }
  });

  // Probe hardware capabilities on startup via Tauri IPC if available
  if (window.__TAURI__) {
    window.__TAURI__.core.invoke('probe_hardware')
      .then(report => {
        const statusText = document.getElementById('engine-status');
        if (statusText) {
          statusText.textContent = `Engine: Active • Hardware Accelerated • ${report.slice(0, 35)}...`;
        }
      })
      .catch(() => {});
  }
});
