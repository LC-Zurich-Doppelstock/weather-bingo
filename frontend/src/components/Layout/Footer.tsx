export default function Footer() {
  return (
    <footer className="border-t border-border bg-surface px-4 py-2 text-center text-xs text-text-muted">
      Weather data:{" "}
      <a
        href="https://www.yr.no"
        target="_blank"
        rel="noopener noreferrer"
        className="text-text-secondary hover:text-primary"
      >
        yr.no (MET Norway)
      </a>
      {" | "}
      Map:{" "}
      <a
        href="https://www.openstreetmap.org"
        target="_blank"
        rel="noopener noreferrer"
        className="text-text-secondary hover:text-primary"
      >
        OpenStreetMap
      </a>
    </footer>
  );
}
