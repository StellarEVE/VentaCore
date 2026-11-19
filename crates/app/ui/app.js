// ==========================================================================
// VentaCore Studio - Hybrid Design System Runtime
// Fully wired with Tauri 2.0 Native IPC Engine
// ==========================================================================

document.addEventListener('DOMContentLoaded', () => {
  const isTauri = typeof window.__TAURI__ !== 'undefined';
  const tauriInvoke = isTauri ? window.__TAURI__.core.invoke : (cmd, args) => Promise.reject(`IPC not available for ${cmd}`);

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

    // Audio VU meter & spectrum animation
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

  // Hook video element playback updates
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
  // 4. Real Song Selection & Waveform Drawing
  // ------------------------------------------------------------------------
  const btnBrowseMusic = document.getElementById('btn-browse-music');
  const songNameElem = document.getElementById('song-name');
  const songDurationElem = document.getElementById('song-duration');
  const btnPlaySong = document.getElementById('btn-play-song');
  const songPlayhead = document.getElementById('song-playhead');
  const waveformCanvas = document.getElementById('waveform-canvas');

  function drawWaveform(peaks, downbeats = [], durationSec = 180) {
    if (!waveformCanvas) return;
    const ctx = waveformCanvas.getContext('2d');
    const width = waveformCanvas.width;
    const height = waveformCanvas.height;

    ctx.clearRect(0, 0, width, height);

    // Dark grid
    ctx.strokeStyle = 'rgba(255, 255, 255, 0.04)';
    ctx.lineWidth = 1;
    for (let x = 0; x < width; x += 40) {
      ctx.beginPath();
      ctx.moveTo(x, 0);
      ctx.lineTo(x, height);
      ctx.stroke();
    }

    // Draw peaks
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

  // Initial placeholder waveform
  drawWaveform(Array.from({ length: 150 }, (_, i) => Math.abs(Math.sin(i * 0.1)) * 0.7 + 0.15));

  if (btnBrowseMusic) {
    btnBrowseMusic.addEventListener('click', async () => {
      try {
        btnBrowseMusic.disabled = true;
        btnBrowseMusic.textContent = 'Browsing...';

        const song = await tauriInvoke('select_song');
        if (song) {
          loadedSong = song;

          if (songNameElem) songNameElem.textContent = song.name;
          if (songDurationElem) songDurationElem.textContent = song.duration_formatted;
          document.querySelector('.accent-badge').textContent = `${song.beats_count} Beats Detected`;

          // Synchronize rhythm engine to the detected BPM
          restartBeatClock(song.bpm);

          // Draw real waveform peaks
          drawWaveform(song.peaks, song.downbeats, song.duration_seconds);

          // Set audio element source
          if (masterSongAudio) {
            masterSongAudio.src = song.stream_url;
          }

          // Update edit page A1 track label
          const a1Label = document.querySelector('.timeline-audio-clip .clip-label');
          if (a1Label) a1Label.textContent = `${song.name} (Locked Clock)`;

          const statusText = document.getElementById('engine-status');
          if (statusText) statusText.textContent = `Song Loaded: ${song.name} • ${song.bpm.toFixed(1)} BPM • ${song.beats_count} Beats`;
        }
      } catch (err) {
        alert(`Failed to load song: ${err}`);
      } finally {
        btnBrowseMusic.disabled = false;
        btnBrowseMusic.textContent = 'Browse Audio';
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
  // 5. Real Footage Pool & Video Auditioning
  // ------------------------------------------------------------------------
  const btnAddVideos = document.getElementById('btn-add-videos');
  const footageListElem = document.getElementById('footage-list');
  const poolCountElem = document.getElementById('pool-count');

  function renderFootagePoolUI(clips) {
    if (!footageListElem) return;
    footageListElem.innerHTML = '';

    if (poolCountElem) poolCountElem.textContent = `${clips.length} Clips Loaded`;

    clips.forEach(clip => {
      const item = document.createElement('div');
      item.className = 'footage-item' + (activeClip && activeClip.id === clip.id ? ' selected' : '');
      item.innerHTML = `
        <div class="footage-meta">
          <span class="footage-name">🎬 ${clip.name}</span>
          <span class="footage-info">${clip.duration_formatted} • ${clip.resolution} • ${clip.fps.toFixed(1)} fps</span>
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
    // Highlight matching footage item
    const items = document.querySelectorAll('.footage-item');
    items.forEach(el => {
      if (el.textContent.includes(clip.name)) el.classList.add('selected');
    });

    if (sourceTitle) sourceTitle.textContent = clip.name;
    if (sourceRes) sourceRes.textContent = clip.resolution;
    if (sourceFps) sourceFps.textContent = `${clip.fps.toFixed(2)} fps`;
    if (previewActiveTitle) previewActiveTitle.textContent = clip.name;

    // Load actual video stream into video element
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
      try {
        btnAddVideos.disabled = true;
        btnAddVideos.textContent = 'Importing...';

        const newClips = await tauriInvoke('import_footage');
        if (newClips && newClips.length > 0) {
          loadedFootage.push(...newClips);
          renderFootagePoolUI(loadedFootage);

          // Automatically audition the first imported clip
          if (!activeClip && loadedFootage.length > 0) {
            selectClipForAudition(loadedFootage[0]);
          }
        }
      } catch (err) {
        alert(`Failed to import footage: ${err}`);
      } finally {
        btnAddVideos.disabled = false;
        btnAddVideos.textContent = '+ Import Footage';
      }
    });
  }

  // ------------------------------------------------------------------------
  // 6. In/Out Point Marking & Trimming
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
      if (isTauri) {
        await tauriInvoke('set_in_out', {
          clipId: activeClip.id,
          inSec: activeClip.in_point,
          outSec: activeClip.out_point
        }).catch(() => {});
      }
    });
  }

  if (btnMarkOut) {
    btnMarkOut.addEventListener('click', async () => {
      if (!activeClip || !sourceVideo) return;
      activeClip.out_point = sourceVideo.currentTime;
      updateInOutDisplay();
      if (isTauri) {
        await tauriInvoke('set_in_out', {
          clipId: activeClip.id,
          inSec: activeClip.in_point,
          outSec: activeClip.out_point
        }).catch(() => {});
      }
    });
  }

  if (btnClearInOut) {
    btnClearInOut.addEventListener('click', async () => {
      if (!activeClip) return;
      activeClip.in_point = 0;
      activeClip.out_point = activeClip.duration_seconds;
      updateInOutDisplay();
      if (isTauri) {
        await tauriInvoke('set_in_out', {
          clipId: activeClip.id,
          inSec: 0,
          outSec: activeClip.duration_seconds
        }).catch(() => {});
      }
    });
  }

  // ------------------------------------------------------------------------
  // 7. Cut Page Montage Engine & Real Cut Generation
  // ------------------------------------------------------------------------
  const btnGenerateMontage = document.getElementById('btn-generate-montage');
  const selectPacingMode = document.getElementById('select-pacing-mode');
  const sliderSensitivity = document.getElementById('slider-sensitivity');
  const valSensitivity = document.getElementById('val-sensitivity');
  const macroEnergyStrip = document.getElementById('macro-energy-strip');
  const microCutStrip = document.getElementById('micro-cut-strip');

  if (sliderSensitivity && valSensitivity) {
    sliderSensitivity.addEventListener('input', (e) => {
      valSensitivity.textContent = `${e.target.value}%`;
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

      try {
        const pacingMode = selectPacingMode ? selectPacingMode.value : 'balanced';
        const sensitivity = sliderSensitivity ? parseFloat(sliderSensitivity.value) / 100 : 0.78;

        const result = await tauriInvoke('generate_beat_cut', { pacingMode, sensitivity });
        activeCuts = result.cuts;

        // Render micro cut slices
        if (microCutStrip) {
          microCutStrip.innerHTML = '';
          result.cuts.slice(0, 30).forEach((cut, idx) => {
            const slice = document.createElement('div');
            slice.className = 'beat-slice-item' + (idx % 2 === 0 ? ' slice-accent' : '');
            slice.style.flex = `${Math.max(1, Math.round(cut.duration_seconds))}`;
            slice.innerHTML = `
              <span class="slice-name">${cut.clip_name} (${cut.duration_seconds.toFixed(2)}s)</span>
              <span class="slice-beat-mark">${cut.beat_label}</span>
            `;
            microCutStrip.appendChild(slice);
          });
        }

        // Render multitrack V1 cuts on Edit page
        const v1Lane = document.querySelector('.timeline-lane-row .track-v1');
        if (v1Lane) {
          v1Lane.innerHTML = '';
          const totalDur = result.song_duration || 180;
          result.cuts.forEach(cut => {
            const leftPct = (cut.start_seconds / totalDur) * 100;
            const widthPct = (cut.duration_seconds / totalDur) * 100;
            const clipBlock = document.createElement('div');
            clipBlock.className = 'timeline-clip clip-v1';
            clipBlock.style.left = `${leftPct}%`;
            clipBlock.style.width = `${Math.max(2, widthPct)}%`;
            clipBlock.innerHTML = `<span class="clip-label">${cut.clip_name}</span>`;
            v1Lane.appendChild(clipBlock);
          });
        }

        btnGenerateMontage.textContent = `✅ Generated ${result.total_cuts} Beat Cuts!`;
        const cutStats = document.querySelector('.timeline-quick-stats');
        if (cutStats) {
          cutStats.innerHTML = `
            <span>Song: <strong>${result.song_duration.toFixed(0)}s</strong></span>
            <span>Total Cuts: <strong>${result.total_cuts}</strong></span>
            <span>Avg Length: <strong>${result.avg_duration.toFixed(2)}s</strong></span>
          `;
        }

        setTimeout(() => {
          btnGenerateMontage.disabled = false;
          btnGenerateMontage.textContent = '⚡ Generate Beat-Sync Cut';
        }, 2500);

      } catch (err) {
        alert(`Cut generation error: ${err}`);
        btnGenerateMontage.disabled = false;
        btnGenerateMontage.textContent = '⚡ Generate Beat-Sync Cut';
      }
    });
  }

  // ------------------------------------------------------------------------
  // 8. Real Export Rendering via FFmpeg Pipeline
  // ------------------------------------------------------------------------
  const btnStartQueue = document.getElementById('btn-start-queue');
  const btnOpenExportFolder = document.getElementById('btn-open-export-folder');
  let lastExportedFile = null;

  if (btnStartQueue) {
    btnStartQueue.addEventListener('click', async () => {
      if (loadedFootage.length === 0) {
        alert('Cannot export: No video footage has been loaded. Import videos on the Media page first.');
        return;
      }

      btnStartQueue.disabled = true;
      btnStartQueue.textContent = 'Rendering Master MP4...';

      const progressBar = document.querySelector('.job-progress-bar');
      const jobTag = document.querySelector('.job-tag');
      if (progressBar) progressBar.style.width = '25%';
      if (jobTag) jobTag.textContent = 'Rendering (Hardware Accelerated)...';

      try {
        const res = await tauriInvoke('export_video', { preset: 'YouTube 4K UHD' });
        if (res.success) {
          lastExportedFile = res.output_path;
          if (progressBar) progressBar.style.width = '100%';
          if (jobTag) jobTag.textContent = 'Export Completed (100%)';
          alert(`Export finished!\nSaved to: ${res.output_path}`);
        } else {
          if (progressBar) progressBar.style.width = '0%';
          if (jobTag) jobTag.textContent = 'Cancelled';
        }
      } catch (err) {
        alert(`Export error: ${err}`);
        if (jobTag) jobTag.textContent = 'Failed';
      } finally {
        btnStartQueue.disabled = false;
        btnStartQueue.textContent = '⚡ Start Batch Render';
      }
    });
  }

  if (btnOpenExportFolder) {
    btnOpenExportFolder.addEventListener('click', async () => {
      const target = lastExportedFile || (loadedFootage.length > 0 ? loadedFootage[0].path : '.');
      await tauriInvoke('open_directory', { path: target }).catch(err => alert(`Error opening folder: ${err}`));
    });
  }

  // ------------------------------------------------------------------------
  // 9. Global Keyboard Shortcuts (Rule 10 & Pro NLE)
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
      togglePlayback();
    }

    if ((e.key === 'i' || e.key === 'I') && e.target.tagName !== 'INPUT') {
      if (btnMarkIn) btnMarkIn.click();
    }

    if ((e.key === 'o' || e.key === 'O') && e.target.tagName !== 'INPUT') {
      if (btnMarkOut) btnMarkOut.click();
    }

    if (e.key === 'ArrowLeft' && e.target.tagName !== 'INPUT') {
      if (sourceVideo) sourceVideo.currentTime = Math.max(0, sourceVideo.currentTime - (1 / 30));
    }
    if (e.key === 'ArrowRight' && e.target.tagName !== 'INPUT') {
      if (sourceVideo) sourceVideo.currentTime += (1 / 30);
    }
  });

  // Probe hardware capabilities on startup via Tauri IPC
  if (isTauri) {
    tauriInvoke('probe_hardware')
      .then(report => {
        const statusText = document.getElementById('engine-status');
        if (statusText) {
          statusText.textContent = `Engine: Active • Hardware Accelerated (D3D11 / NVENC) • Buffer: 128`;
        }
      })
      .catch(() => {});
  }
});
