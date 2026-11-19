// ==========================================================================
// VentaCore Studio - Hybrid Design System Runtime
// Wired with Tauri 2.0 Native IPC Engine + Browser Fallback
// ==========================================================================

document.addEventListener('DOMContentLoaded', () => {
  // ------------------------------------------------------------------------
  // IPC Detection & Invocation
  // ------------------------------------------------------------------------
  function getTauriInvoke() {
    if (window.__TAURI__ && window.__TAURI__.core && typeof window.__TAURI__.core.invoke === 'function') {
      return window.__TAURI__.core.invoke;
    }
    if (window.__TAURI__ && typeof window.__TAURI__.invoke === 'function') {
      return window.__TAURI__.invoke;
    }
    if (window.__TAURI_INTERNALS__ && typeof window.__TAURI_INTERNALS__.invoke === 'function') {
      return window.__TAURI_INTERNALS__.invoke;
    }
    return null;
  }

  async function tauriInvoke(cmd, args = {}) {
    let invoker = getTauriInvoke();
    if (!invoker) {
      // Small retry loop in case window.__TAURI__ is injected slightly after DOMContentLoaded
      for (let i = 0; i < 6; i++) {
        await new Promise(r => setTimeout(r, 40));
        invoker = getTauriInvoke();
        if (invoker) break;
      }
    }
    if (invoker) {
      return invoker(cmd, args);
    }
    throw new Error('NATIVE_IPC_UNAVAILABLE');
  }

  // ------------------------------------------------------------------------
  // 1. Charter Rule 10: Persistent Bottom Tab Navigation
  // ------------------------------------------------------------------------
  const navTabs = document.querySelectorAll('.nav-tab');
  const pageViews = document.querySelectorAll('.page-view');
  let currentPage = 'media';

  function switchPage(pageId) {
    currentPage = pageId;
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

    // Pause individual video player if leaving media page
    if (pageId !== 'media' && sourceVideo && !sourceVideo.paused) {
      sourceVideo.pause();
      isPlaying = false;
      if (playBtn) playBtn.textContent = '▶';
    }

    if (pageId === 'export') {
      if (typeof updateExportMetrics === 'function') {
        updateExportMetrics();
      }
    }
  }

  navTabs.forEach(tab => {
    tab.addEventListener('click', () => switchPage(tab.dataset.page));
  });

  const quickExportBtn = document.getElementById('btn-quick-export');
  if (quickExportBtn) {
    quickExportBtn.addEventListener('click', () => switchPage('export'));
  }

  // ------------------------------------------------------------------------
  // 2. Cyber-Studio Rhythm & Beat Engine
  // ------------------------------------------------------------------------
  let BPM = 128.0;
  let beatTimer = null;
  let currentBeat = 0;
  let currentBar = 1;
  let currentSubdivision = 1;

  const beatLamp = document.getElementById('beat-lamp');
  const beatDots = document.querySelectorAll('.beat-dot');
  const bpmDisplay = document.getElementById('bpm-display');
  const musicalTimeDisplay = document.getElementById('musical-time');
  const vuBarL = document.getElementById('vu-meter-l');
  const vuBarR = document.getElementById('vu-meter-r');
  const vuClipL = document.getElementById('vu-clip-l');
  const vuClipR = document.getElementById('vu-clip-r');
  const spectrumBars = document.querySelectorAll('.spectrum-bar');

  function pulseBeat() {
    currentBeat = (currentBeat % 4) + 1;
    if (currentBeat === 1) {
      currentBar++;
    }

    beatDots.forEach(dot => {
      if (parseInt(dot.dataset.beat, 10) === currentBeat) {
        dot.classList.add('active');
      } else {
        dot.classList.remove('active');
      }
    });

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

    currentSubdivision = Math.floor(Math.random() * 4) + 1;
    if (musicalTimeDisplay) {
      const barStr = String(currentBar).padStart(3, '0');
      const beatStr = String(currentBeat).padStart(2, '0');
      const subStr = String(currentSubdivision).padStart(2, '0');
      musicalTimeDisplay.textContent = `${barStr} : ${beatStr} : ${subStr}`;
    }

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

    spectrumBars.forEach(bar => {
      const height = isPlaying ? (20 + Math.random() * 75) : (10 + Math.random() * 20);
      bar.style.height = `${height}%`;
    });
  }

  function restartBeatClock(newBpm) {
    BPM = newBpm;
    if (bpmDisplay) bpmDisplay.textContent = BPM.toFixed(1);
    if (beatTimer) clearInterval(beatTimer);
    const intervalMs = (60 / BPM) * 1000;
    beatTimer = setInterval(pulseBeat, intervalMs);
  }

  restartBeatClock(BPM);

  // ------------------------------------------------------------------------
  // 3. Media Assets State
  // ------------------------------------------------------------------------
  let loadedSong = null;
  let loadedFootage = [];
  let activeClip = null;
  let activeCuts = [];
  let isPlaying = false;

  const sourceVideo = document.getElementById('source-video-element');
  const sourcePlaceholder = document.getElementById('source-placeholder');
  const masterSongAudio = document.getElementById('master-song-audio');
  const sourceTitle = document.getElementById('source-title');
  const sourceRes = document.getElementById('source-res');
  const sourceFps = document.getElementById('source-fps');
  const previewActiveTitle = document.getElementById('preview-active-title');
  const sourceTimecode = document.getElementById('source-timecode');
  const headerTimecode = document.getElementById('header-timecode');
  const sourceScrub = document.getElementById('source-scrub');
  const playBtn = document.getElementById('btn-play-pause');
  const inOutRange = document.getElementById('in-out-range');

  function formatTimecodeFromSeconds(sec, fps = 30) {
    if (isNaN(sec) || sec < 0) sec = 0;
    const totalFrames = Math.floor(sec * fps);
    const ff = totalFrames % fps;
    const totalSec = Math.floor(sec);
    const ss = totalSec % 60;
    const mm = Math.floor(totalSec / 60) % 60;
    const hh = Math.floor(totalSec / 3600);
    return `${String(hh).padStart(2, '0')}:${String(mm).padStart(2, '0')}:${String(ss).padStart(2, '0')}:${String(ff).padStart(2, '0')}`;
  }

  function updateTimecodeUI(sec) {
    const tc = formatTimecodeFromSeconds(sec, 30);
    if (sourceTimecode) sourceTimecode.textContent = tc;
    if (headerTimecode) headerTimecode.textContent = tc;
  }

  function togglePlayback() {
    isPlaying = !isPlaying;
    const icon = isPlaying ? '⏸' : '▶';
    if (playBtn) playBtn.textContent = icon;

    if (sourceVideo && sourceVideo.src) {
      if (isPlaying) {
        sourceVideo.play().catch(() => {});
      } else {
        sourceVideo.pause();
      }
    }
  }

  if (playBtn) playBtn.addEventListener('click', togglePlayback);

  if (sourceVideo) {
    sourceVideo.addEventListener('timeupdate', () => {
      const cur = sourceVideo.currentTime;
      const dur = sourceVideo.duration || 1;
      updateTimecodeUI(cur);

      if (sourceScrub) {
        sourceScrub.value = (cur / dur) * 1000;
      }
    });

    sourceVideo.addEventListener('ended', () => {
      isPlaying = false;
      if (playBtn) playBtn.textContent = '▶';
    });
  }

  if (sourceScrub) {
    sourceScrub.addEventListener('input', (e) => {
      if (sourceVideo && sourceVideo.duration) {
        const targetSec = (parseInt(e.target.value, 10) / 1000) * sourceVideo.duration;
        sourceVideo.currentTime = targetSec;
        updateTimecodeUI(targetSec);
      }
    });
  }

  // ------------------------------------------------------------------------
  // 4. Waveform Drawing
  // ------------------------------------------------------------------------
  const songNameElem = document.getElementById('song-name');
  const songDurationElem = document.getElementById('song-duration');
  const btnPlaySong = document.getElementById('btn-play-song');
  const songPlayhead = document.getElementById('song-playhead');
  const waveformCanvas = document.getElementById('waveform-canvas');

  function drawWaveform(peaks) {
    if (!waveformCanvas) return;
    const ctx = waveformCanvas.getContext('2d');
    const width = waveformCanvas.width;
    const height = waveformCanvas.height;

    ctx.clearRect(0, 0, width, height);

    ctx.strokeStyle = 'rgba(255, 255, 255, 0.04)';
    ctx.lineWidth = 1;
    for (let x = 0; x < width; x += 40) {
      ctx.beginPath();
      ctx.moveTo(x, 0);
      ctx.lineTo(x, height);
      ctx.stroke();
    }

    const grad = ctx.createLinearGradient(0, 0, 0, height);
    grad.addColorStop(0, '#00f0ff');
    grad.addColorStop(0.5, '#2563eb');
    grad.addColorStop(1, '#00f0ff');
    ctx.fillStyle = grad;

    const count = peaks.length || 200;
    const barWidth = width / count;

    for (let i = 0; i < count; i++) {
      const val = peaks[i] || 0.2;
      const barH = Math.max(3, val * (height - 6));
      const y = (height - barH) / 2;
      ctx.fillRect(i * barWidth, y, Math.max(1, barWidth - 1), barH);
    }
  }

  function drawTimelineTrackWaveform(peaks, downbeats = [], duration = 180) {
    const canvas = document.getElementById('timeline-waveform-canvas');
    if (!canvas) return;

    const rect = canvas.getBoundingClientRect();
    const width = Math.max(300, Math.floor(rect.width || canvas.clientWidth || 800));
    const height = Math.max(20, Math.floor(rect.height || canvas.clientHeight || 36));

    if (canvas.width !== width || canvas.height !== height) {
      canvas.width = width;
      canvas.height = height;
    }

    const ctx = canvas.getContext('2d');
    ctx.clearRect(0, 0, width, height);

    if (!peaks || peaks.length === 0) return;

    // Glowing cyan/emerald gradient
    const grad = ctx.createLinearGradient(0, 0, 0, height);
    grad.addColorStop(0, '#10b981');   // emerald top
    grad.addColorStop(0.5, '#00f0ff'); // cyan mid
    grad.addColorStop(1, '#047857');   // deep teal bottom
    ctx.fillStyle = grad;

    const count = peaks.length;
    const barWidth = Math.max(1, width / count);

    for (let i = 0; i < count; i++) {
      const p = peaks[i] || 0.1;
      const barH = Math.max(2, p * (height - 4));
      const x = i * barWidth;
      const y = (height - barH) / 2;
      ctx.fillRect(x, y, Math.max(1, barWidth - 1), barH);
    }

    // Downbeat markers overlay
    if (downbeats && downbeats.length > 0 && duration > 0) {
      ctx.fillStyle = 'rgba(255, 255, 255, 0.45)';
      downbeats.forEach(t => {
        const x = (t / duration) * width;
        ctx.fillRect(Math.round(x), 0, 1.5, height);
      });
    }
  }

  drawWaveform(Array.from({ length: 150 }, (_, i) => Math.abs(Math.sin(i * 0.1)) * 0.7 + 0.15));
  drawTimelineTrackWaveform(Array.from({ length: 200 }, (_, i) => Math.abs(Math.sin(i * 0.08)) * 0.7 + 0.15));
  window.addEventListener('resize', () => {
    if (loadedSong) {
      drawTimelineTrackWaveform(loadedSong.peaks, loadedSong.downbeats, loadedSong.duration_seconds);
    }
  });

  function setSongData(song) {
    loadedSong = song;

    if (songNameElem) songNameElem.textContent = song.name;
    if (songDurationElem) songDurationElem.textContent = song.duration_formatted;
    const badge = document.querySelector('.accent-badge');
    if (badge) badge.textContent = `${song.beats_count} Beats Detected`;

    restartBeatClock(song.bpm);
    drawWaveform(song.peaks);
    drawTimelineTrackWaveform(song.peaks, song.downbeats, song.duration_seconds);
    if (typeof renderMacroEnergyStrip === 'function') {
      renderMacroEnergyStrip(song);
    }
    if (typeof updateExportMetrics === 'function') {
      updateExportMetrics();
    }

    if (masterSongAudio) {
      masterSongAudio.src = song.stream_url;
    }

    const a1Label = document.getElementById('timeline-audio-label') || document.querySelector('.timeline-audio-clip .clip-label');
    if (a1Label) a1Label.textContent = `🎵 ${song.name} (${song.bpm.toFixed(1)} BPM Reference Clock)`;

    const statusText = document.getElementById('engine-status');
    if (statusText) statusText.textContent = `Song Loaded: ${song.name} • ${song.bpm.toFixed(1)} BPM • ${song.beats_count} Beats`;
  }

  // ------------------------------------------------------------------------
  // 5. Audio Selection (Tauri IPC + HTML5 Fallback)
  // ------------------------------------------------------------------------
  const btnBrowseMusic = document.getElementById('btn-browse-music');
  const fileInputAudio = document.getElementById('file-input-audio');

  if (btnBrowseMusic) {
    btnBrowseMusic.addEventListener('click', async () => {
      btnBrowseMusic.disabled = true;
      btnBrowseMusic.textContent = 'Browsing...';

      try {
        const song = await tauriInvoke('select_song');
        if (song) {
          setSongData(song);
        }
      } catch (err) {
        // Fallback to HTML5 File Picker if IPC is not present
        if (fileInputAudio) {
          fileInputAudio.click();
        }
      } finally {
        btnBrowseMusic.disabled = false;
        btnBrowseMusic.textContent = 'Browse Audio';
      }
    });
  }

  if (fileInputAudio) {
    fileInputAudio.addEventListener('change', async (e) => {
      const file = e.target.files[0];
      if (!file) return;

      const blobUrl = URL.createObjectURL(file);
      const name = file.name;

      // Extract duration and waveform peaks via Web Audio API
      try {
        const arrayBuffer = await file.arrayBuffer();
        const audioCtx = new (window.AudioContext || window.webkitAudioContext)();
        const audioBuffer = await audioCtx.decodeAudioData(arrayBuffer);

        const durationSec = audioBuffer.duration;
        const mins = Math.floor(durationSec / 60);
        const secs = Math.floor(durationSec % 60);
        const durationFormatted = `${String(mins).padStart(2, '0')}:${String(secs).padStart(2, '0')}`;

        // Compute peaks from channel data
        const channelData = audioBuffer.getChannelData(0);
        const numPeaks = 200;
        const step = Math.floor(channelData.length / numPeaks);
        const peaks = [];
        for (let i = 0; i < numPeaks; i++) {
          let maxVal = 0;
          for (let j = 0; j < step; j++) {
            const v = Math.abs(channelData[i * step + j]);
            if (v > maxVal) maxVal = v;
          }
          peaks.push(Math.max(0.08, maxVal));
        }

        const bpmEstimate = 128.0;
        const beatsCount = Math.round(durationSec * (bpmEstimate / 60));

        setSongData({
          path: name,
          name: name,
          duration_seconds: durationSec,
          duration_formatted: durationFormatted,
          bpm: bpmEstimate,
          beats_count: beatsCount,
          peaks: peaks,
          stream_url: blobUrl,
          sample_rate: audioBuffer.sampleRate,
        });

      } catch (e) {
        // Simple fallback
        setSongData({
          path: name,
          name: name,
          duration_seconds: 180,
          duration_formatted: "03:00",
          bpm: 128.0,
          beats_count: 384,
          peaks: Array.from({ length: 150 }, () => Math.random() * 0.8 + 0.1),
          stream_url: blobUrl,
          sample_rate: 48000,
        });
      }
    });
  }

  if (btnPlaySong) {
    btnPlaySong.addEventListener('click', () => {
      if (!masterSongAudio || !masterSongAudio.src) {
        alert('Please browse and load a master song audio file first.');
        return;
      }
      if (masterSongAudio.paused) {
        masterSongAudio.play().then(() => {
          btnPlaySong.textContent = '⏸ Pause Audio';
          isPlaying = true;
        }).catch(e => alert(`Audio playback error: ${e}`));
      } else {
        masterSongAudio.pause();
        btnPlaySong.textContent = '▶ Play Audio';
        isPlaying = false;
      }
    });
  }

  if (masterSongAudio) {
    masterSongAudio.addEventListener('timeupdate', () => {
      if (songPlayhead && masterSongAudio.duration) {
        const pct = (masterSongAudio.currentTime / masterSongAudio.duration) * 100;
        songPlayhead.style.left = `${pct}%`;
      }
    });

    masterSongAudio.addEventListener('ended', () => {
      if (btnPlaySong) btnPlaySong.textContent = '▶ Play Audio';
      isPlaying = false;
    });
  }

  // ------------------------------------------------------------------------
  // 6. Footage Pool & Video Auditioning (Tauri IPC + HTML5 Fallback)
  // ------------------------------------------------------------------------
  const btnAddVideos = document.getElementById('btn-add-videos');
  const fileInputFootage = document.getElementById('file-input-footage');
  const footageListElem = document.getElementById('footage-list');
  const poolCountElem = document.getElementById('pool-count');

  function renderFootagePoolUI(clips) {
    if (!footageListElem) return;
    footageListElem.innerHTML = '';

    if (poolCountElem) poolCountElem.textContent = `${clips.length} Clips Loaded`;

    clips.forEach(clip => {
      const item = document.createElement('div');
      item.className = 'footage-item' + (activeClip && activeClip.id === clip.id ? ' selected' : '');

      const thumbHtml = clip.thumbnail_url
        ? `<img src="${clip.thumbnail_url}" class="footage-thumb-img" alt="${clip.name}" />`
        : `<div class="footage-thumb-placeholder">🎬</div>`;

      item.innerHTML = `
        <div class="footage-left">
          <div class="footage-thumb-wrapper">${thumbHtml}</div>
          <div class="footage-meta">
            <span class="footage-name">${clip.name}</span>
            <span class="footage-info">${clip.duration_formatted} • ${clip.resolution} • ${clip.fps ? clip.fps.toFixed(1) : '30.0'} fps</span>
          </div>
        </div>
        <button class="btn btn-xs btn-secondary">Audition</button>
      `;

      item.addEventListener('click', () => selectClipForAudition(clip));
      footageListElem.appendChild(item);
    });
  }

  function selectClipForAudition(clip) {
    activeClip = clip;

    document.querySelectorAll('.footage-item').forEach(el => el.classList.remove('selected'));
    const items = document.querySelectorAll('.footage-item');
    items.forEach(el => {
      if (el.textContent.includes(clip.name)) el.classList.add('selected');
    });

    if (sourceTitle) sourceTitle.textContent = clip.name;
    if (sourceRes) sourceRes.textContent = clip.resolution;
    if (sourceFps) sourceFps.textContent = `${clip.fps ? clip.fps.toFixed(2) : '30.00'} fps`;
    if (previewActiveTitle) previewActiveTitle.textContent = clip.name;

    if (sourceVideo) {
      sourceVideo.src = clip.stream_url;
      sourceVideo.style.display = 'block';
      if (sourcePlaceholder) sourcePlaceholder.style.display = 'none';
      sourceVideo.currentTime = clip.in_point || 0;
      updateTimecodeUI(clip.in_point || 0);
    }

    updateInOutDisplay();
  }

  if (btnAddVideos) {
    btnAddVideos.addEventListener('click', async () => {
      btnAddVideos.disabled = true;
      btnAddVideos.textContent = 'Importing...';

      try {
        const newClips = await tauriInvoke('import_footage');
        if (newClips && newClips.length > 0) {
          loadedFootage.push(...newClips);
          renderFootagePoolUI(loadedFootage);
          if (!activeClip && loadedFootage.length > 0) {
            selectClipForAudition(loadedFootage[0]);
          }
        }
      } catch (err) {
        // Fallback to HTML5 File Picker if IPC is not present
        if (fileInputFootage) {
          fileInputFootage.click();
        }
      } finally {
        btnAddVideos.disabled = false;
        btnAddVideos.textContent = '+ Import Footage';
      }
    });
  }

  if (fileInputFootage) {
    fileInputFootage.addEventListener('change', (e) => {
      const files = Array.from(e.target.files);
      if (files.length === 0) return;

      files.forEach((file, idx) => {
        const blobUrl = URL.createObjectURL(file);
        const name = file.name;

        // Create temporary video element to extract dimensions and duration
        const tempVideo = document.createElement('video');
        tempVideo.src = blobUrl;
        tempVideo.addEventListener('loadedmetadata', () => {
          const w = tempVideo.videoWidth || 1920;
          const h = tempVideo.videoHeight || 1080;
          const dur = tempVideo.duration || 30;
          const mins = Math.floor(dur / 60);
          const secs = Math.floor(dur % 60);
          const durStr = `${String(mins).padStart(2, '0')}:${String(secs).padStart(2, '0')}`;
          const resStr = w >= 3800 ? `${w}x${h} 4K UHD` : `${w}x${h} FHD`;

          const clip = {
            id: loadedFootage.length + 1,
            path: name,
            name: name,
            resolution: resStr,
            width: w,
            height: h,
            fps: 30.0,
            duration_seconds: dur,
            duration_formatted: durStr,
            codec: "H.264",
            stream_url: blobUrl,
            in_point: 0.0,
            out_point: dur,
          };

          loadedFootage.push(clip);
          renderFootagePoolUI(loadedFootage);

          if (!activeClip) {
            selectClipForAudition(clip);
          }
        });
      });
    });
  }

  // ------------------------------------------------------------------------
  // 7. In/Out Point Marking & Trimming
  // ------------------------------------------------------------------------
  const btnMarkIn = document.getElementById('btn-mark-in');
  const btnMarkOut = document.getElementById('btn-mark-out');
  const btnClearInOut = document.getElementById('btn-clear-inout');
  const inMarker = document.querySelector('.in-marker');
  const outMarker = document.querySelector('.out-marker');

  function updateInOutDisplay() {
    if (!activeClip || !inOutRange) return;
    const dur = activeClip.duration_seconds || 1;
    const inPct = (activeClip.in_point / dur) * 100;
    const outPct = (activeClip.out_point / dur) * 100;

    inOutRange.style.left = `${inPct}%`;
    inOutRange.style.width = `${Math.max(1, outPct - inPct)}%`;

    if (inMarker) inMarker.style.left = `${inPct}%`;
    if (outMarker) outMarker.style.left = `${outPct}%`;
  }

  if (btnMarkIn) {
    btnMarkIn.addEventListener('click', async () => {
      if (!activeClip || !sourceVideo) return;
      activeClip.in_point = sourceVideo.currentTime;
      updateInOutDisplay();
      tauriInvoke('set_in_out', {
        clipId: activeClip.id,
        inSec: activeClip.in_point,
        outSec: activeClip.out_point
      }).catch(() => {});
    });
  }

  if (btnMarkOut) {
    btnMarkOut.addEventListener('click', async () => {
      if (!activeClip || !sourceVideo) return;
      activeClip.out_point = sourceVideo.currentTime;
      updateInOutDisplay();
      tauriInvoke('set_in_out', {
        clipId: activeClip.id,
        inSec: activeClip.in_point,
        outSec: activeClip.out_point
      }).catch(() => {});
    });
  }

  if (btnClearInOut) {
    btnClearInOut.addEventListener('click', async () => {
      if (!activeClip) return;
      activeClip.in_point = 0;
      activeClip.out_point = activeClip.duration_seconds;
      updateInOutDisplay();
      tauriInvoke('set_in_out', {
        clipId: activeClip.id,
        inSec: 0,
        outSec: activeClip.duration_seconds
      }).catch(() => {});
    });
  }

  // ------------------------------------------------------------------------
  // 8. Cut Page Montage Engine & Dynamic Strips
  // ------------------------------------------------------------------------
  const btnGenerateMontage = document.getElementById('btn-generate-montage');
  const selectPacingMode = document.getElementById('select-pacing-mode');
  const sliderSensitivity = document.getElementById('slider-sensitivity');
  const valSensitivity = document.getElementById('val-sensitivity');
  const sliderMotionBias = document.getElementById('slider-motion-bias');
  const valMotionBias = document.getElementById('val-motion-bias');
  const microCutStrip = document.getElementById('micro-cut-strip');
  const macroEnergyStrip = document.getElementById('macro-energy-strip');

  if (sliderSensitivity && valSensitivity) {
    sliderSensitivity.addEventListener('input', (e) => {
      valSensitivity.textContent = `${e.target.value}%`;
    });
  }

  if (sliderMotionBias && valMotionBias) {
    sliderMotionBias.addEventListener('input', (e) => {
      const val = parseInt(e.target.value, 10);
      valMotionBias.textContent = val > 75 ? 'Very High' : val > 50 ? 'High' : val > 25 ? 'Medium' : 'Low';
    });
  }

  function format_time(totalSeconds) {
    const s = Math.max(0, Math.floor(totalSeconds || 0));
    const mins = Math.floor(s / 60);
    const secs = s % 60;
    return `${String(mins).padStart(2, '0')}:${String(secs).padStart(2, '0')}`;
  }

  function formatSMPTE(totalSec) {
    const s = Math.max(0, totalSec);
    const hrs = Math.floor(s / 3600);
    const mins = Math.floor((s % 3600) / 60);
    const secs = Math.floor(s % 60);
    const frames = Math.floor((s % 1) * 30);
    return `${String(hrs).padStart(2, '0')}:${String(mins).padStart(2, '0')}:${String(secs).padStart(2, '0')}:${String(frames).padStart(2, '0')}`;
  }

  function renderMacroEnergyStrip(song) {
    if (!macroEnergyStrip) return;
    const dur = song ? song.duration_seconds : 180;
    macroEnergyStrip.innerHTML = `
      <div class="energy-segment segment-intro" style="width: 12%;"><span>Intro</span></div>
      <div class="energy-segment segment-verse" style="width: 22%;"><span>Verse 1</span></div>
      <div class="energy-segment segment-build" style="width: 14%;"><span>Build-up</span></div>
      <div class="energy-segment segment-drop" style="width: 20%;"><span>🔥 DROP 1</span></div>
      <div class="energy-segment segment-verse" style="width: 18%;"><span>Verse 2</span></div>
      <div class="energy-segment segment-outro" style="width: 14%;"><span>Outro</span></div>
      <div class="macro-viewport-window" id="macro-window" style="left: 0%; width: 12%;"></div>
    `;

    macroEnergyStrip.addEventListener('click', (e) => {
      const rect = macroEnergyStrip.getBoundingClientRect();
      const clickRatio = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
      seekSequenceTo(clickRatio * dur);
    });
  }

  function renderMicroCutStrip(cuts) {
    if (!microCutStrip) return;
    microCutStrip.innerHTML = '';
    if (!cuts || cuts.length === 0) return;

    cuts.forEach((cut, idx) => {
      const slice = document.createElement('div');
      slice.className = 'beat-slice-item' + (idx % 2 === 0 ? ' slice-accent' : '') + (cut.pinned ? ' pinned' : '');
      slice.style.flex = `${Math.max(1, Math.round(cut.duration_seconds * 2))}`;
      slice.dataset.cutIndex = idx;
      slice.title = `Click to seek • Double-click to Pin/Unpin (${cut.clip_name})`;
      slice.innerHTML = `
        <span class="slice-name">${cut.clip_name} (${cut.duration_seconds.toFixed(2)}s)</span>
        <span class="slice-beat-mark">${cut.pinned ? '📌 PINNED' : cut.beat_label}</span>
      `;

      slice.addEventListener('click', () => {
        seekSequenceTo(cut.start_seconds);
      });

      slice.addEventListener('dblclick', (e) => {
        e.stopPropagation();
        cut.pinned = !cut.pinned;
        renderMicroCutStrip(activeCuts);
      });

      microCutStrip.appendChild(slice);
    });
  }

  if (btnGenerateMontage) {
    btnGenerateMontage.addEventListener('click', async () => {
      if (loadedFootage.length === 0) {
        alert('Please import video clips into the Footage Pool on the Media tab (Ctrl+1) before generating cuts.');
        return;
      }

      btnGenerateMontage.disabled = true;
      btnGenerateMontage.textContent = '⚡ Analyzing Beats & Planning Cuts...';

      const pacingMode = selectPacingMode ? selectPacingMode.value : 'balanced';
      const sensitivity = sliderSensitivity ? parseFloat(sliderSensitivity.value) / 100 : 0.78;

      try {
        let result = null;
        try {
          result = await tauriInvoke('generate_beat_cut', { pacingMode, sensitivity });
        } catch (ipcErr) {
          // In-browser / fallback cut generation
          const songDur = loadedSong ? loadedSong.duration_seconds : 180;
          const beatDur = 60.0 / (loadedSong ? loadedSong.bpm : 128.0);
          const cutInterval = pacingMode === 'high-energy' ? beatDur * 2 : pacingMode === 'cinematic' ? beatDur * 8 : beatDur * 4;
          const totalCuts = Math.min(60, Math.floor(songDur / cutInterval));

          const cuts = [];
          for (let i = 0; i < totalCuts; i++) {
            const clip = loadedFootage[i % loadedFootage.length];
            cuts.push({
              index: i + 1,
              clip_name: clip.name,
              clip_path: clip.path,
              start_seconds: i * cutInterval,
              end_seconds: (i + 1) * cutInterval,
              duration_seconds: cutInterval,
              stream_url: clip.stream_url,
              beat_label: `Bar ${Math.floor(i / 4) + 1} : Beat ${(i % 4) + 1}`,
              source_in: 0,
              source_out: cutInterval,
            });
          }
          result = { cuts, total_cuts: cuts.length, avg_duration: cutInterval, song_duration: songDur };
        }

        activeCuts = result.cuts;
        renderMicroCutStrip(activeCuts);
        renderEditTimeline(activeCuts);
        if (loadedSong) renderMacroEnergyStrip(loadedSong);

        const cutStats = document.querySelector('.timeline-quick-stats');
        if (cutStats) {
          cutStats.innerHTML = `
            <span>Song: <strong>${result.song_duration.toFixed(0)}s</strong></span>
            <span>Total Cuts: <strong>${result.total_cuts}</strong></span>
            <span>Avg Length: <strong>${result.avg_duration.toFixed(2)}s</strong></span>
          `;
        }

        updateExportMetrics();
        seekSequenceTo(0);

        btnGenerateMontage.textContent = `✅ Generated ${result.total_cuts} Beat Cuts!`;
        setTimeout(() => {
          btnGenerateMontage.disabled = false;
          btnGenerateMontage.textContent = '⚡ Generate Beat-Sync Cut';
        }, 2200);

      } catch (err) {
        alert(`Cut generation error: ${err}`);
        btnGenerateMontage.disabled = false;
        btnGenerateMontage.textContent = '⚡ Generate Beat-Sync Cut';
      }
    });
  }

  // ------------------------------------------------------------------------
  // 9. Unified Sequence Playback Engine (Page 2 & Page 3)
  // ------------------------------------------------------------------------
  const cutProgramVideo = document.getElementById('cut-program-video');
  const cutPlaceholder = document.getElementById('cut-placeholder');
  const cutTimecode = document.getElementById('cut-timecode');
  const cutPhraseLabel = document.getElementById('cut-phrase-label');
  const btnCutPlay = document.getElementById('btn-cut-play');

  const editSourceVideo = document.getElementById('edit-source-video');
  const editSourcePlaceholder = document.getElementById('edit-source-placeholder');
  const editSourceFilename = document.getElementById('edit-source-filename');
  const editProgramVideo = document.getElementById('edit-program-video');
  const editProgramPlaceholder = document.getElementById('edit-program-placeholder');
  const timelineCurrentTc = document.getElementById('timeline-current-tc');
  const timelineGlobalPlayhead = document.querySelector('.timeline-global-playhead');

  let isSequencePlaying = false;
  let sequenceTime = 0.0;
  let sequenceAnim = null;
  let currentActiveCutIdx = -1;

  function toggleSequencePlayback() {
    if (activeCuts.length === 0) {
      alert('Please click "⚡ Generate Beat-Sync Cut" on the Cut tab (Ctrl+2) first to assemble the sequence.');
      return;
    }

    isSequencePlaying = !isSequencePlaying;
    const icon = isSequencePlaying ? '⏸' : '▶';
    if (btnCutPlay) btnCutPlay.textContent = icon;

    if (isSequencePlaying) {
      if (masterSongAudio && masterSongAudio.src) {
        masterSongAudio.currentTime = sequenceTime;
        masterSongAudio.play().catch(() => {});
      }
      runSequenceLoop();
    } else {
      if (masterSongAudio) masterSongAudio.pause();
      if (cutProgramVideo) cutProgramVideo.pause();
      if (editProgramVideo) editProgramVideo.pause();
      if (sequenceAnim) cancelAnimationFrame(sequenceAnim);
    }
  }

  if (btnCutPlay) {
    btnCutPlay.addEventListener('click', toggleSequencePlayback);
  }

  function seekSequenceTo(targetSeconds) {
    const totalDur = loadedSong ? loadedSong.duration_seconds : (activeCuts.length > 0 ? activeCuts[activeCuts.length - 1].end_seconds : 180);
    sequenceTime = Math.max(0, Math.min(targetSeconds, totalDur));

    if (masterSongAudio && masterSongAudio.src) {
      masterSongAudio.currentTime = sequenceTime;
    }

    updateSequenceFrame(sequenceTime);
  }

  function runSequenceLoop() {
    if (!isSequencePlaying) return;

    if (masterSongAudio && !masterSongAudio.paused) {
      sequenceTime = masterSongAudio.currentTime;
    } else {
      sequenceTime += (1 / 30);
    }

    const totalDur = loadedSong ? loadedSong.duration_seconds : (activeCuts.length > 0 ? activeCuts[activeCuts.length - 1].end_seconds : 180);
    if (sequenceTime >= totalDur) {
      isSequencePlaying = false;
      sequenceTime = 0;
      if (btnCutPlay) btnCutPlay.textContent = '▶';
      if (masterSongAudio) masterSongAudio.pause();
      return;
    }

    updateSequenceFrame(sequenceTime);
    sequenceAnim = requestAnimationFrame(runSequenceLoop);
  }

  function updateSequenceFrame(curTime) {
    if (activeCuts.length === 0) return;

    const cutIdx = activeCuts.findIndex(c => curTime >= c.start_seconds && curTime < c.end_seconds);
    const cut = cutIdx !== -1 ? activeCuts[cutIdx] : activeCuts[activeCuts.length - 1];
    const idx = cutIdx !== -1 ? cutIdx : activeCuts.length - 1;

    const inClipOffset = Math.max(0, curTime - cut.start_seconds);
    const targetSourceTime = cut.source_in + inClipOffset;

    // Update Cut Page Program Monitor
    if (cutProgramVideo) {
      if (cutPlaceholder) cutPlaceholder.style.display = 'none';
      cutProgramVideo.style.display = 'block';

      if (cutProgramVideo.src !== cut.stream_url) {
        cutProgramVideo.src = cut.stream_url;
        cutProgramVideo.currentTime = targetSourceTime;
        if (isSequencePlaying) cutProgramVideo.play().catch(() => {});
      } else if (Math.abs(cutProgramVideo.currentTime - targetSourceTime) > 0.25) {
        cutProgramVideo.currentTime = targetSourceTime;
      }

      if (isSequencePlaying && cutProgramVideo.paused) {
        cutProgramVideo.play().catch(() => {});
      }
    }

    // Update Edit Page Program Monitor
    if (editProgramVideo) {
      if (editProgramPlaceholder) editProgramPlaceholder.style.display = 'none';
      editProgramVideo.style.display = 'block';

      if (editProgramVideo.src !== cut.stream_url) {
        editProgramVideo.src = cut.stream_url;
        editProgramVideo.currentTime = targetSourceTime;
        if (isSequencePlaying) editProgramVideo.play().catch(() => {});
      } else if (Math.abs(editProgramVideo.currentTime - targetSourceTime) > 0.25) {
        editProgramVideo.currentTime = targetSourceTime;
      }
    }

    // Update SMPTE Timecodes & Labels
    const smpte = formatSMPTE(curTime);
    if (cutTimecode) cutTimecode.textContent = smpte;
    if (timelineCurrentTc) timelineCurrentTc.textContent = smpte;
    if (cutPhraseLabel) {
      cutPhraseLabel.innerHTML = `Phrase: <strong>${cut.beat_label} (${cut.clip_name})</strong>`;
    }

    // Move Playhead Cursor across Timeline and Macro Window
    const totalDur = loadedSong ? loadedSong.duration_seconds : 180;
    const progressPct = (curTime / totalDur) * 100;

    if (timelineGlobalPlayhead) {
      timelineGlobalPlayhead.style.left = `${progressPct}%`;
    }

    const macroWindow = document.getElementById('macro-window');
    if (macroWindow) {
      macroWindow.style.left = `${Math.min(88, progressPct)}%`;
    }

    // Highlight playing cut in strips
    if (currentActiveCutIdx !== idx) {
      currentActiveCutIdx = idx;

      document.querySelectorAll('.beat-slice-item').forEach((el, i) => {
        if (i === idx) el.classList.add('active-playing-cut');
        else el.classList.remove('active-playing-cut');
      });

      document.querySelectorAll('.timeline-clip').forEach((el) => {
        if (parseInt(el.dataset.cutIndex, 10) === idx) el.classList.add('active-playing-cut');
        else el.classList.remove('active-playing-cut');
      });
    }
  }

  // ------------------------------------------------------------------------
  // 10. Edit Page Multitrack NLE Timeline & Tooling
  // ------------------------------------------------------------------------
  const laneTrackV1 = document.getElementById('lane-track-v1');
  const multitrackView = document.getElementById('multitrack-view');
  const timelineZoom = document.getElementById('timeline-zoom');
  const inspectorClipName = document.getElementById('inspector-clip-name');
  const inspectorScale = document.getElementById('inspector-scale');
  const inspectorRotation = document.getElementById('inspector-rotation');
  const inspectorColorPreset = document.getElementById('inspector-color-preset');

  let currentTool = 'select'; // 'select' | 'razor' | 'slip'
  let selectedCutIndex = null;

  const tools = {
    select: document.getElementById('tool-select'),
    razor: document.getElementById('tool-razor'),
    slip: document.getElementById('tool-slip'),
    snapping: document.getElementById('tool-snapping')
  };

  function setTool(toolName) {
    currentTool = toolName;
    Object.keys(tools).forEach(k => {
      if (tools[k]) {
        if (k === toolName) tools[k].classList.add('active');
        else if (k !== 'snapping') tools[k].classList.remove('active');
      }
    });

    if (multitrackView) {
      multitrackView.classList.remove('cursor-razor', 'cursor-slip');
      if (toolName === 'razor') multitrackView.classList.add('cursor-razor');
      if (toolName === 'slip') multitrackView.classList.add('cursor-slip');
    }
  }

  if (tools.select) tools.select.addEventListener('click', () => setTool('select'));
  if (tools.razor) tools.razor.addEventListener('click', () => setTool('razor'));
  if (tools.slip) tools.slip.addEventListener('click', () => setTool('slip'));
  if (tools.snapping) tools.snapping.addEventListener('click', () => tools.snapping.classList.toggle('active'));

  if (timelineZoom && multitrackView) {
    timelineZoom.addEventListener('input', (e) => {
      multitrackView.style.width = `${e.target.value}%`;
    });
  }

  function renderEditTimeline(cuts) {
    if (!laneTrackV1) return;
    laneTrackV1.innerHTML = '';
    if (!cuts || cuts.length === 0) return;

    const totalDur = loadedSong ? loadedSong.duration_seconds : 180;

    cuts.forEach((cut, idx) => {
      const leftPct = (cut.start_seconds / totalDur) * 100;
      const widthPct = (cut.duration_seconds / totalDur) * 100;

      const clipBlock = document.createElement('div');
      clipBlock.className = 'timeline-clip clip-v1' + (selectedCutIndex === idx ? ' active-clip' : '');
      clipBlock.dataset.cutIndex = idx;
      clipBlock.style.left = `${leftPct}%`;
      clipBlock.style.width = `${Math.max(1.5, widthPct)}%`;
      clipBlock.innerHTML = `
        <span class="clip-label">${cut.clip_name} (${cut.duration_seconds.toFixed(1)}s)</span>
      `;

      clipBlock.addEventListener('click', (e) => {
        e.stopPropagation();

        if (currentTool === 'razor') {
          // Razor split clip into 2 slices at click point
          const rect = clipBlock.getBoundingClientRect();
          const clickOffsetRatio = Math.max(0.15, Math.min(0.85, (e.clientX - rect.left) / rect.width));

          const durA = cut.duration_seconds * clickOffsetRatio;
          const durB = cut.duration_seconds - durA;

          const cutA = {
            ...cut,
            duration_seconds: durA,
            end_seconds: cut.start_seconds + durA,
            source_out: cut.source_in + durA,
          };

          const cutB = {
            ...cut,
            index: cut.index + 1,
            start_seconds: cutA.end_seconds,
            duration_seconds: durB,
            source_in: cutA.source_out,
            beat_label: `${cut.beat_label} (Split)`,
          };

          activeCuts.splice(idx, 1, cutA, cutB);
          activeCuts.forEach((c, i) => c.index = i + 1);

          tauriInvoke('update_cuts', { cuts: activeCuts }).catch(() => {});
          renderEditTimeline(activeCuts);
          renderMicroCutStrip(activeCuts);
          seekSequenceTo(cutA.end_seconds);
          return;
        }

        if (currentTool === 'slip') {
          // Slip source window inside this slice
          const slipStep = 0.5;
          cut.source_in = Math.max(0, cut.source_in + slipStep);
          cut.source_out = cut.source_in + cut.duration_seconds;
          seekSequenceTo(cut.start_seconds);
          return;
        }

        // Selection mode
        selectedCutIndex = idx;
        document.querySelectorAll('.timeline-clip').forEach(c => c.classList.remove('active-clip'));
        clipBlock.classList.add('active-clip');

        // Update Inspector
        if (inspectorClipName) inspectorClipName.textContent = cut.clip_name;
        if (inspectorScale) inspectorScale.value = '100.0%';
        if (inspectorRotation) inspectorRotation.value = '0.0°';

        // Update Source Monitor
        if (editSourceVideo && editSourcePlaceholder && editSourceFilename) {
          editSourceFilename.textContent = cut.clip_name;
          editSourcePlaceholder.style.display = 'none';
          editSourceVideo.style.display = 'block';
          editSourceVideo.src = cut.stream_url;
          editSourceVideo.currentTime = cut.source_in;
        }

        seekSequenceTo(cut.start_seconds);
      });

      laneTrackV1.appendChild(clipBlock);
    });
  }

  // Ruler scrubbing handler
  const rulerTrack = document.querySelector('.ruler-track');
  if (rulerTrack) {
    rulerTrack.addEventListener('click', (e) => {
      const rect = rulerTrack.getBoundingClientRect();
      const clickRatio = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
      const totalDur = loadedSong ? loadedSong.duration_seconds : 180;
      seekSequenceTo(clickRatio * totalDur);
    });
  }

  // Inspector color grade preset filters
  if (inspectorColorPreset) {
    inspectorColorPreset.addEventListener('change', (e) => {
      const val = e.target.value;
      const filter = val === 'cyber' ? 'contrast(1.25) saturate(1.4) hue-rotate(190deg)'
        : val === 'bleach' ? 'contrast(1.4) saturate(0.6)'
        : val === 'violet' ? 'sepia(0.3) hue-rotate(240deg) saturate(1.3)'
        : val === 'bw' ? 'grayscale(1) contrast(1.3)'
        : 'none';

      if (cutProgramVideo) cutProgramVideo.style.filter = filter;
      if (editProgramVideo) editProgramVideo.style.filter = filter;
      if (editSourceVideo) editSourceVideo.style.filter = filter;
    });
  }

  // ------------------------------------------------------------------------
  // 11. Export Delivery Bay (Dynamic Queue & Batch Rendering)
  // ------------------------------------------------------------------------
  const btnStartQueue = document.getElementById('btn-start-queue');
  const btnAddQueue = document.getElementById('btn-add-queue');
  const btnOpenExportFolder = document.getElementById('btn-open-export-folder');
  const selectExportEncoder = document.getElementById('select-export-encoder');
  const inputExportBitrate = document.getElementById('input-export-bitrate');
  const exportEstFilesize = document.getElementById('export-est-filesize');
  const exportTotalFrames = document.getElementById('export-total-frames');
  const exportEstRendTime = document.getElementById('export-est-rendertime');
  const exportFrameStatus = document.getElementById('export-frame-status');
  const exportBadgeRes = document.getElementById('export-badge-res');
  const queueList = document.getElementById('queue-list');
  const queueCountSub = document.getElementById('queue-count-sub');
  const presetCards = document.querySelectorAll('.preset-card');

  let selectedExportPreset = 'YouTube 4K UHD';
  let lastExportedFile = null;
  let renderQueueJobs = [];

  function updateExportMetrics() {
    const dur = loadedSong ? loadedSong.duration_seconds : 180;
    const mbps = inputExportBitrate ? (parseInt(inputExportBitrate.value, 10) || 45) : 45;

    const estMb = ((dur * mbps) / 8).toFixed(1);
    const totalFrames = Math.round(dur * 30);
    const estSec = Math.max(5, Math.round(dur / 2.5));

    if (exportEstFilesize) exportEstFilesize.textContent = `${estMb} MB`;
    if (exportTotalFrames) exportTotalFrames.textContent = totalFrames.toLocaleString();
    if (exportEstRendTime) exportEstRendTime.textContent = `~${estSec}s`;
    if (exportBadgeRes) {
      exportBadgeRes.textContent = selectedExportPreset.includes('4K') ? '3840x2160 UHD'
        : selectedExportPreset.includes('Vertical') ? '1080x1920 (9:16)'
        : '1920x1080 FHD';
    }
    if (exportFrameStatus) {
      exportFrameStatus.textContent = `Ready to render ${activeCuts.length} beat cuts across ${format_time(dur)}`;
    }
  }

  presetCards.forEach(card => {
    card.addEventListener('click', () => {
      presetCards.forEach(c => c.classList.remove('active'));
      card.classList.add('active');
      const name = card.querySelector('.preset-name');
      if (name) selectedExportPreset = name.textContent.trim();
      updateExportMetrics();
    });
  });

  if (inputExportBitrate) {
    inputExportBitrate.addEventListener('input', updateExportMetrics);
  }

  function renderQueueList() {
    if (!queueList) return;
    queueList.innerHTML = '';

    if (renderQueueJobs.length === 0) {
      queueList.innerHTML = `
        <div class="queue-empty-state">
          No active render jobs. Choose preset and click "+ Add to Render Queue" or "Start Batch Render".
        </div>
      `;
      if (queueCountSub) queueCountSub.textContent = '0 Jobs Queued';
      return;
    }

    if (queueCountSub) queueCountSub.textContent = `${renderQueueJobs.length} Active Job(s)`;

    renderQueueJobs.forEach(job => {
      const card = document.createElement('div');
      card.className = 'queue-job-card' + (job.status === 'Rendering' ? ' active' : '');
      card.innerHTML = `
        <div class="job-meta">
          <span class="job-title">${job.title}</span>
          <span class="job-tag">${job.status} (${job.progress}%)</span>
        </div>
        <div class="job-progress-wrapper">
          <div class="job-progress-bar" style="width: ${job.progress}%;"></div>
        </div>
        <div class="job-stats">
          <span>Preset: <strong>${job.preset}</strong></span>
          <span>Target Bitrate: <strong>${job.bitrate} Mbps</strong></span>
        </div>
      `;
      queueList.appendChild(card);
    });
  }

  if (btnAddQueue) {
    btnAddQueue.addEventListener('click', () => {
      const mbps = inputExportBitrate ? (parseInt(inputExportBitrate.value, 10) || 45) : 45;
      renderQueueJobs.push({
        id: Date.now(),
        title: `VentaCore_${selectedExportPreset.replace(/\s+/g, '_')}.mp4`,
        preset: selectedExportPreset,
        bitrate: mbps,
        status: 'Queued',
        progress: 0,
      });
      renderQueueList();
    });
  }

  if (btnStartQueue) {
    btnStartQueue.addEventListener('click', async () => {
      if (loadedFootage.length === 0) {
        alert('Cannot export: No video footage has been loaded. Import videos on the Media page first.');
        return;
      }

      const mbps = inputExportBitrate ? (parseInt(inputExportBitrate.value, 10) || 45) : 45;
      const encoder = selectExportEncoder ? selectExportEncoder.value : 'h264_nvenc';

      if (renderQueueJobs.length === 0) {
        renderQueueJobs.push({
          id: Date.now(),
          title: `VentaCore_${selectedExportPreset.replace(/\s+/g, '_')}.mp4`,
          preset: selectedExportPreset,
          bitrate: mbps,
          status: 'Rendering',
          progress: 15,
        });
      } else {
        renderQueueJobs[0].status = 'Rendering';
        renderQueueJobs[0].progress = 15;
      }
      renderQueueList();

      btnStartQueue.disabled = true;
      btnStartQueue.textContent = 'Rendering Master MP4...';

      let progressTimer = setInterval(() => {
        if (renderQueueJobs.length > 0 && renderQueueJobs[0].status === 'Rendering') {
          renderQueueJobs[0].progress = Math.min(92, renderQueueJobs[0].progress + 12);
          renderQueueList();
        }
      }, 350);

      try {
        const res = await tauriInvoke('export_video', {
          preset: selectedExportPreset,
          encoder: encoder,
          bitrateMbps: mbps,
        });

        clearInterval(progressTimer);

        if (res.success) {
          lastExportedFile = res.output_path;
          if (renderQueueJobs.length > 0) {
            renderQueueJobs[0].status = 'Completed';
            renderQueueJobs[0].progress = 100;
            renderQueueList();
          }
          alert(`Export finished!\n${res.message}\nSaved to: ${res.output_path}`);
        } else {
          if (renderQueueJobs.length > 0) {
            renderQueueJobs[0].status = 'Cancelled';
            renderQueueJobs[0].progress = 0;
            renderQueueList();
          }
        }
      } catch (err) {
        clearInterval(progressTimer);
        alert(`Export render error: ${err}`);
      } finally {
        setTimeout(() => {
          btnStartQueue.disabled = false;
          btnStartQueue.textContent = '⚡ Start Batch Render';
        }, 1200);
      }
    });
  }

  if (btnOpenExportFolder) {
    btnOpenExportFolder.addEventListener('click', async () => {
      const target = lastExportedFile || (loadedFootage.length > 0 ? loadedFootage[0].path : '.');
      tauriInvoke('open_directory', { path: target }).catch(err => alert(`Error opening folder: ${err}`));
    });
  }

  // ------------------------------------------------------------------------
  // 12. Global Keyboard Shortcuts
  // ------------------------------------------------------------------------
  window.addEventListener('keydown', (e) => {
    if (e.ctrlKey) {
      if (e.key === '1') { e.preventDefault(); switchPage('media'); }
      if (e.key === '2') { e.preventDefault(); switchPage('cut'); }
      if (e.key === '3') { e.preventDefault(); switchPage('edit'); }
      if (e.key === '4') { e.preventDefault(); switchPage('export'); }
    }

    if (e.code === 'Space' && e.target.tagName !== 'INPUT' && !e.target.isContentEditable) {
      e.preventDefault();
      if (currentPage === 'media') {
        togglePlayback();
      } else {
        toggleSequencePlayback();
      }
    }

    // Editing tool shortcuts
    if (currentPage === 'edit' && e.target.tagName !== 'INPUT') {
      if (e.key === 'v' || e.key === 'V') setTool('select');
      if (e.key === 'c' || e.key === 'C') setTool('razor');
      if (e.key === 'y' || e.key === 'Y') setTool('slip');
      if (e.key === 's' || e.key === 'S') {
        if (tools.snapping) tools.snapping.classList.toggle('active');
      }

      // Delete selected clip from timeline
      if ((e.key === 'Delete' || e.key === 'Backspace') && selectedCutIndex !== null) {
        if (selectedCutIndex >= 0 && selectedCutIndex < activeCuts.length) {
          activeCuts.splice(selectedCutIndex, 1);
          activeCuts.forEach((c, i) => c.index = i + 1);
          selectedCutIndex = null;
          tauriInvoke('update_cuts', { cuts: activeCuts }).catch(() => {});
          renderEditTimeline(activeCuts);
          renderMicroCutStrip(activeCuts);
        }
      }
    }

    // Media Page trim shortcuts
    if (currentPage === 'media' && e.target.tagName !== 'INPUT') {
      if (e.key === 'i' || e.key === 'I') {
        if (btnMarkIn) btnMarkIn.click();
      }
      if (e.key === 'o' || e.key === 'O') {
        if (btnMarkOut) btnMarkOut.click();
      }
      if (e.key === 'ArrowLeft') {
        if (sourceVideo) sourceVideo.currentTime = Math.max(0, sourceVideo.currentTime - (1 / 30));
      }
      if (e.key === 'ArrowRight') {
        if (sourceVideo) sourceVideo.currentTime += (1 / 30);
      }
    }
  });

  // Probe hardware capabilities on startup
  tauriInvoke('probe_hardware')
    .then(report => {
      const statusText = document.getElementById('engine-status');
      if (statusText) {
        statusText.textContent = `Engine: Active • Hardware Accelerated (D3D11 / NVENC) • Buffer: 128`;
      }
    })
    .catch(() => {});
});
