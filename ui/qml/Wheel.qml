pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import NashiApp

// The pie menu. Geometry comes from WheelGeometry so drawing and hit-testing
// agree: slice 0 is centred at 12 o'clock, slices go clockwise.
Item {
    id: pie

    property int stage: OverlayController.Hidden
    property string mode: "convert"
    property Selection selection: null
    property real centerX: width / 2
    property real centerY: height / 2
    property int hoveredIndex: -1
    property bool overCenter: false
    // Settings.wheelScale; scales both radii so drawing and hit-testing agree.
    property real wheelScale: 1.0

    readonly property WheelModel model: selection ? selection.model : null
    readonly property bool showItems: (stage === OverlayController.Live || stage === OverlayController.Pick)
                                      && selection !== null && selection.ready
    readonly property int count: showItems && model ? model.count : 0
    readonly property real innerRadius: WheelGeometry.scaledInnerRadius(wheelScale)
    readonly property real outerRadius: showItems ? WheelGeometry.scaledOuterRadius(count, wheelScale)
                                                  : WheelGeometry.scaled(118, wheelScale)
    readonly property var hoveredItem: hoveredIndex >= 0 && model ? model.get(hoveredIndex) : null

    signal sliceClicked(int index)
    signal cancelClicked()

    anchors.fill: parent

    function modeTitle(m) {
        if (m === "tools") return qsTr("Tools")
        if (m === "actions") return qsTr("Actions")
        return qsTr("Convert")
    }

    function dropHint(m) {
        if (m === "tools") return qsTr("Drop here for tools")
        if (m === "actions") return qsTr("Drop here for file actions")
        return qsTr("Drop here to convert")
    }

    // Returns the slice index, -1 (centre: cancel) or -2 (outside).
    function sliceAt(x, y) {
        return WheelGeometry.hit(x - centerX, y - centerY, count, innerRadius, outerRadius)
    }

    function trackPointer(x, y) {
        const s = sliceAt(x, y)
        overCenter = s === -1
        hoveredIndex = showItems ? s : -1
    }

    // Keyboard: move the highlight by `step` over the slices that can run.
    function moveHighlight(step) {
        if (count === 0 || !model)
            return
        let i = hoveredIndex < 0 ? (step > 0 ? -1 : 0) : hoveredIndex
        for (let n = 0; n < count; ++n) {
            i = (i + step + count) % count
            if (model.get(i).available) {
                overCenter = false
                hoveredIndex = i
                return
            }
        }
    }

    function activateHighlight() {
        if (hoveredIndex >= 0 && hoveredIndex < count)
            sliceClicked(hoveredIndex)
    }

    Item {
        id: disc
        x: pie.centerX - pie.outerRadius
        y: pie.centerY - pie.outerRadius
        width: pie.outerRadius * 2
        height: pie.outerRadius * 2
        scale: pie.visible || !Theme.motion ? 1 : 0.85
        Behavior on scale { NumberAnimation { duration: Theme.quick; easing.type: Easing.OutCubic } }

        Rectangle {
            anchors.fill: parent
            radius: width / 2
            color: Qt.rgba(Theme.canvas.r, Theme.canvas.g, Theme.canvas.b, 0.94)
            border.color: Qt.rgba(0.5, 0.5, 0.5, 0.17)
            border.width: 0.7
        }

        // Stage 1: one big drop target labelled with the mode.
        Rectangle {
            visible: !pie.showItems
            anchors.centerIn: parent
            width: parent.width - 16
            height: width
            radius: width / 2
            color: Qt.rgba(Theme.gold.r, Theme.gold.g, Theme.gold.b, 0.22)
            border.color: Theme.gold
            border.width: 2

            NashiMark {
                anchors.centerIn: parent
                anchors.verticalCenterOffset: -parent.height * 0.22
                size: parent.width * 0.22
                happy: true
                opacity: 0.9
            }
        }

        Repeater {
            model: pie.showItems ? pie.model : null
            delegate: WheelSlice {
                id: sliceDelegate
                required property int index
                required property string title
                required property string iconName
                required property bool available
                required property string missing
                anchors.fill: parent
                sliceIndex: sliceDelegate.index
                sliceCount: pie.count
                innerRadius: pie.innerRadius
                outerRadius: pie.outerRadius
                label: sliceDelegate.title
                icon: sliceDelegate.iconName
                enabledItem: sliceDelegate.available
                missingText: sliceDelegate.missing
                hovered: pie.hoveredIndex === sliceDelegate.index
                uppercase: pie.mode === "convert"
                uiScale: pie.wheelScale
            }
        }

        // Centre: cancel zone and status.
        Rectangle {
            anchors.centerIn: parent
            width: pie.innerRadius * 2 - 6
            height: width
            radius: width / 2
            color: pie.overCenter ? Qt.rgba(0.85, 0.2, 0.2, 0.85) : Theme.surface
            border.color: Theme.border

            BusyIndicator {
                anchors.centerIn: parent
                width: parent.width * 0.7
                height: width
                running: pie.selection !== null && pie.selection.loading
                visible: running
            }
            Label {
                anchors.centerIn: parent
                width: parent.width - 8
                visible: !(pie.selection !== null && pie.selection.loading)
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.Wrap
                font.pixelSize: Math.round(WheelGeometry.scaled(12, pie.wheelScale))
                font.weight: Font.DemiBold
                color: pie.overCenter ? "white" : Theme.text
                text: pie.overCenter || pie.stage === OverlayController.Pick ? qsTr("Cancel")
                                                                                 : pie.modeTitle(pie.mode)
            }
        }
    }

    // Caption under the wheel: file summary, hovered item, errors.
    Label {
        id: caption
        x: pie.centerX - width / 2
        y: Math.min(pie.height - height - 8, disc.y + disc.height + 10)
        width: Math.min(420, pie.width - 16)
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.Wrap
        padding: 8
        color: Theme.text
        font.pixelSize: Theme.sizeSmall + 1
        font.weight: Font.Medium
        background: Rectangle {
            radius: 12
            color: Qt.rgba(Theme.surface.r, Theme.surface.g, Theme.surface.b, 0.96)
            border.color: Theme.border
        }
        text: {
            if (!pie.selection)
                return ""
            if (pie.selection.error.length > 0)
                return qsTr("Could not read the file types: %1").arg(pie.selection.error)
            if (pie.hoveredItem && !pie.hoveredItem.available)
                return qsTr("%1 needs %2 — install it from the Tools page").arg(pie.hoveredItem.title).arg(pie.hoveredItem.missing)
            if (pie.hoveredItem)
                return pie.hoveredItem.title
            if (!pie.showItems)
                return pie.stage === OverlayController.Pick ? qsTr("Reading files…") : pie.dropHint(pie.mode)
            if (pie.count === 0)
                return qsTr("Nothing to offer for %1").arg(pie.selection.summary)
            return pie.stage === OverlayController.Pick ? qsTr("%1 — click an option").arg(pie.selection.summary)
                                                          : pie.selection.summary
        }
        visible: pie.visible && text.length > 0
    }

    // Pick stage: hover + click selection using the same hit test.
    MouseArea {
        x: disc.x
        y: disc.y
        width: disc.width
        height: disc.height
        enabled: pie.visible && pie.stage === OverlayController.Pick
        hoverEnabled: true
        onPositionChanged: (mouse) => pie.trackPointer(mouse.x + x, mouse.y + y)
        onExited: { pie.hoveredIndex = -1; pie.overCenter = false }
        onClicked: (mouse) => {
            const s = pie.sliceAt(mouse.x + x, mouse.y + y)
            if (s === -1 || s === -2)
                pie.cancelClicked()
            else if (pie.showItems)
                pie.sliceClicked(s)
        }
    }
}
