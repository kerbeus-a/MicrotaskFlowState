import { useRef, useCallback } from "react";
import "./RecordButton.css";

interface RecordButtonProps {
  isRecording: boolean;
  isProcessing?: boolean;
  recordingTime?: number;
  onStartRecording: () => void;
  onStopRecording: () => void;
}

export default function RecordButton({
  isRecording,
  isProcessing = false,
  recordingTime = 0,
  onStartRecording,
  onStopRecording,
}: RecordButtonProps) {
  const isHoldingRef = useRef(false);
  const pointerIdRef = useRef<number | null>(null);

  const formatTime = (seconds: number): string => {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins}:${secs.toString().padStart(2, '0')}`;
  };

  const handleStart = useCallback((e: React.MouseEvent | React.PointerEvent | React.TouchEvent) => {
    if (isProcessing || isRecording) return;
    
    e.preventDefault();
    e.stopPropagation();
    isHoldingRef.current = true;
    
    // Track pointer ID for pointer events
    if ('pointerId' in e) {
      pointerIdRef.current = e.pointerId;
    }
    
    // Start recording immediately on press
    onStartRecording();
  }, [isProcessing, isRecording, onStartRecording]);

  const handleStop = useCallback((e: React.MouseEvent | React.PointerEvent | React.TouchEvent) => {
    // Only stop if this is the same pointer that started
    if ('pointerId' in e && pointerIdRef.current !== null && e.pointerId !== pointerIdRef.current) {
      return;
    }
    
    // If recording, always stop (don't check isHoldingRef in case it got reset)
    if (!isRecording || isProcessing) return;
    
    e.preventDefault();
    e.stopPropagation();
    isHoldingRef.current = false;
    pointerIdRef.current = null;
    
    // Stop recording on release
    console.log('Stop handler: Stopping recording');
    onStopRecording();
  }, [isRecording, isProcessing, onStopRecording]);

  const handleLeave = useCallback((e: React.MouseEvent | React.PointerEvent | React.TouchEvent) => {
    // If pointer leaves button while pressed, stop recording
    if (isHoldingRef.current && isRecording) {
      e.preventDefault();
      e.stopPropagation();
      isHoldingRef.current = false;
      pointerIdRef.current = null;
      onStopRecording();
    }
  }, [isRecording, onStopRecording]);

  // Click handler - if recording, stop it (fallback for when press-and-hold fails)
  const handleClick = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    
    // If recording, stop it (fallback mechanism)
    if (isRecording && !isProcessing) {
      console.log('Click handler: Stopping recording (fallback)');
      isHoldingRef.current = false;
      pointerIdRef.current = null;
      onStopRecording();
    }
  }, [isRecording, isProcessing, onStopRecording]);

  // Prevent context menu on long press
  const handleContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
  }, []);

  return (
    <div className="record-button-container">
      <button
        className={`record-button ${isRecording ? "recording" : ""} ${isProcessing ? "processing" : ""}`}
        // Pointer events (modern browsers, touch screens)
        onPointerDown={handleStart}
        onPointerUp={handleStop}
        onPointerLeave={handleLeave}
        onPointerCancel={handleStop}
        // Mouse events (fallback for desktop)
        onMouseDown={handleStart}
        onMouseUp={handleStop}
        onMouseLeave={handleLeave}
        // Touch events (mobile)
        onTouchStart={handleStart}
        onTouchEnd={handleStop}
        onTouchCancel={handleStop}
        // Prevent click from interfering
        onClick={handleClick}
        onContextMenu={handleContextMenu}
        disabled={isProcessing}
        title={
          isProcessing
            ? "Processing..."
            : isRecording
            ? "Release to stop"
            : "Hold to record (Win+Shift+R)"
        }
      >
        {isProcessing ? (
          <div className="spinner" />
        ) : (
          <svg
            width="48"
            height="48"
            viewBox="0 0 24 24"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
          >
            {isRecording ? (
              <rect x="6" y="6" width="12" height="12" rx="2" fill="currentColor" />
            ) : (
              <circle cx="12" cy="12" r="10" fill="currentColor" />
            )}
          </svg>
        )}
      </button>
      <div className="recording-hint">
        {isProcessing ? null : isRecording ? "● Recording..." : "Hold to record"}
      </div>
      {isRecording && !isProcessing && (
        <div className="recording-time">
          {formatTime(recordingTime)}
        </div>
      )}
      {isProcessing && (
        <div className="processing-indicator">
          <span>Processing...</span>
        </div>
      )}
    </div>
  );
}
