import "./TimerBar.css";

interface TimerBarProps {
  remaining: number;
  duration: number; // in minutes
}

export default function TimerBar({ remaining, duration }: TimerBarProps) {
  const totalSeconds = duration * 60;
  const progress = totalSeconds > 0 ? (remaining / totalSeconds) * 100 : 0;
  const minutes = Math.floor(remaining / 60);
  const seconds = remaining % 60;

  return (
    <div className="timer-bar">
      <div className="timer-progress" style={{ width: `${progress}%` }} />
      <div className="timer-text">
        {minutes}:{seconds.toString().padStart(2, "0")}
      </div>
    </div>
  );
}
