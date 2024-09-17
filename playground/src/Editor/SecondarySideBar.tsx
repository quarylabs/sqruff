import SideBar, { SideBarEntry } from "./SideBar";
import { FormatIcon, LineageIcon, StructureIcon } from "./Icons";
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

      <SideBarEntry
        title="CST"
        position={"right"}
        selected={selected === SecondaryTool.Cst}
        onClick={() => onSelected(SecondaryTool.Cst)}
      >
        <StructureIcon />
      </SideBarEntry>

      <SideBarEntry
        title="Lineage"
        position={"right"}
        selected={selected === SecondaryTool.Lineage}
        onClick={() => onSelected(SecondaryTool.Lineage)}
      >
        <LineageIcon />
      </SideBarEntry>
    </SideBar>
  );
}
