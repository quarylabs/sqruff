import classNames from "classnames";

export default function Header({
  onNewIssue: onNewIssue,
}: {
  onNewIssue?: () => void;
}) {
  return (
    <div
      className={classNames(
        "w-full",
        "flex",
        "justify-between",
        "pl-5",
        "sm:pl-1",
      )}
    >
      <a
        className="py-4 pl-2 mr-6 flex items-center space-x-2"
        href="https://www.quary.dev/"
      >
        <img
          alt="quary"
          fetchPriority="high"
          width="136"
          height="32"
          decoding="async"
          data-nimg="1"
          className="h-6 w-6"
          style={{ color: "transparent" }}
          src="https://www.quary.dev/images/logos/quary.svg"
        />
        <span className="hidden font-bold sm:inline-block">Quary</span>
      </a>

      <div className="flex items-center min-w-0">
        <button className={classNames()} onClick={onNewIssue}>
          <pre>New issue</pre>
        </button>
      </div>
    </div>
  );
}
