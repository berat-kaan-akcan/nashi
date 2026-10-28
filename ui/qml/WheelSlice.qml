import QtQuick
import QtQuick.Controls
import QtQuick.Shapes
import NashiApp

// One annular sector of the wheel, filling its parent (a square whose centre
// is the wheel centre).
Item {
    id: slice

    property int sliceIndex: 0
    property int sliceCount: 1
    property real innerRadius: 50
    property real outerRadius: 150
    property string label: ""
    property string icon: ""
    property bool enabledItem: true
    property string missingText: ""
    property bool hovered: false
    property color accent: "steelblue"
    property color textColor: "black"
    property color highlightedTextColor: "white"
    property color baseColor: "white"

    readonly property real cx: width / 2
    readonly property real cy: height / 2
    readonly property real start: WheelGeometry.startAngle(sliceIndex, sliceCount)
    readonly property real sweep: WheelGeometry.sweep(sliceCount)
    readonly property real midRad: (start + sweep / 2) * Math.PI / 180
    readonly property real labelRadius: (innerRadius + outerRadius) / 2 + 4
    readonly property real gap: sliceCount > 1 ? 1.2 : 0

    Shape {
        anchors.fill: parent
        preferredRendererType: Shape.CurveRenderer
        ShapePath {
            strokeColor: Qt.rgba(slice.textColor.r, slice.textColor.g, slice.textColor.b, 0.12)
            strokeWidth: 1
            fillColor: slice.hovered && slice.enabledItem ? slice.accent
                       : slice.hovered ? Qt.rgba(slice.textColor.r, slice.textColor.g, slice.textColor.b, 0.12)
                       : Qt.rgba(slice.baseColor.r, slice.baseColor.g, slice.baseColor.b, 0.55)
            PathAngleArc {
                centerX: slice.cx
                centerY: slice.cy
                radiusX: slice.outerRadius - 4
                radiusY: slice.outerRadius - 4
                startAngle: slice.start + slice.gap
                sweepAngle: slice.sweep - 2 * slice.gap
                moveToStart: true
            }
            PathAngleArc {
                centerX: slice.cx
                centerY: slice.cy
                radiusX: slice.innerRadius + 2
                radiusY: slice.innerRadius + 2
                startAngle: slice.start + slice.sweep - slice.gap
                sweepAngle: -(slice.sweep - 2 * slice.gap)
                moveToStart: false
            }
        }
    }

    Column {
        id: content
        width: Math.min(96, 2 * Math.PI * slice.labelRadius / Math.max(1, slice.sliceCount) - 6)
        x: slice.cx + Math.cos(slice.midRad) * slice.labelRadius - width / 2
        y: slice.cy + Math.sin(slice.midRad) * slice.labelRadius - height / 2
        spacing: 2
        opacity: slice.enabledItem ? 1.0 : 0.45

        Image {
            anchors.horizontalCenter: parent.horizontalCenter
            visible: slice.icon.length > 0
            source: slice.icon.length > 0 ? "image://icon/" + slice.icon : ""
            sourceSize: Qt.size(22, 22)
            width: 22
            height: 22
        }
        Label {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            elide: Text.ElideRight
            maximumLineCount: 2
            wrapMode: Text.Wrap
            font.pixelSize: 12
            font.bold: slice.hovered
            color: slice.hovered && slice.enabledItem ? slice.highlightedTextColor : slice.textColor
            text: slice.label
        }
        Label {
            width: parent.width
            visible: !slice.enabledItem && slice.missingText.length > 0
            horizontalAlignment: Text.AlignHCenter
            elide: Text.ElideRight
            font.pixelSize: 9
            color: slice.textColor
            text: qsTr("needs %1").arg(slice.missingText)
        }
    }
}
