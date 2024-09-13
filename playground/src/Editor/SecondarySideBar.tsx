import SideBar, { SideBarEntry } from "./SideBar";
import { FormatIcon } from "./Icons";
import { SecondaryTool } from "./SecondaryPanel";

interface RightSideBarProps {
  selected: SecondaryTool | null;
  onSelected(tool: SecondaryTool): void;
}

export default function SecondarySideBar({
  selected,
  onSelected,
}: RightSideBarProps) {
  return (
    <SideBar position="right">
      <SideBarEntry
        title="Format"
        position={"right"}
        selected={selected === SecondaryTool.Format}
        onClick={() => onSelected(SecondaryTool.Format)}
      >
        <FormatIcon />
      </SideBarEntry>
    </SideBar>
  );
}
