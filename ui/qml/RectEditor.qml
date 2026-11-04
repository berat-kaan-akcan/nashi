pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import NashiApp

// Interactive rectangle editor for cropImage, cropVideo, redactImage, redactVideo.
// Maps cleanly between fitted viewport coordinates and source pixel coordinates.
Item {
    id: root
    objectName: "rectEditor"

    property string filePath: ""
    property string mode: "crop" // "crop" | "redact"
    property bool isVideo: false
    property int mediaWidth: 0
    property int mediaHeight: 0
    property real duration: 0
    property real frameTime: 0
    property int stageHeight: 320

    // Single-rect state (mode === "crop")
    property int rectX: 0
    property int rectY: 0
    property int rectW: 0
    property int rectH: 0
    property string aspectLabel: "free"
    property real aspectRatio: 0

    // Multi-rect state (mode === "redact")
    // Each entry: { x: int, y: int, w: int, h: int, style: "black"|"blur"|"pixelate" }
    property var rects: []
    property int selectedIndex: -1
    property string currentStyle: "black"

    // Internal preview loading state
    property int previewReqId: -1
    property int previewWidth: 0
    property int previewHeight: 0
    property string resolvedImageUrl: ""
    property string fallbackPreviewPath: ""
    property bool loadingPreview: false

    readonly property int effWidth: mediaWidth > 0 ? mediaWidth
                                  : (previewWidth > 0 ? previewWidth
                                  : (previewImg.implicitWidth > 0 ? previewImg.implicitWidth : 640))
    readonly property int effHeight: mediaHeight > 0 ? mediaHeight
                                   : (previewHeight > 0 ? previewHeight
                                   : (previewImg.implicitHeight > 0 ? previewImg.implicitHeight : 360))

    signal cropChanged(int x, int y, int w, int h)
    signal rectsModified()

    implicitWidth: 640
    implicitHeight: mainCol.implicitHeight

    function fileUrl(path) {
        if (!path || path.length === 0)
            return ""
        return "file://" + String(path).split("/").map(encodeURIComponent).join("/")
    }

    function isBuiltInRaster(path) {
        return /\.(png|jpe?g|bmp)$/i.test(String(path))
    }

    function formatTime(sec) {
        const safe = Math.max(0, isFinite(sec) ? sec : 0)
        const m = Math.floor(safe / 60)
        const s = safe % 60
        return m + ":" + (s < 10 ? "0" : "") + s.toFixed(1)
    }

    function styleTitle(st) {
        if (st === "blur")
            return qsTr("Blur")
        if (st === "pixelate")
            return qsTr("Pixelate")
        return qsTr("Black")
    }

    function resetEditor() {
        previewReqId = -1
        previewWidth = 0
        previewHeight = 0
        resolvedImageUrl = ""
        fallbackPreviewPath = ""
        loadingPreview = false
        frameTime = 0
        aspectLabel = "free"
        aspectRatio = 0
        rectX = 0
        rectY = 0
        rectW = 0
        rectH = 0
        rects = []
        selectedIndex = -1
        currentStyle = "black"
        requestFramePreview()
    }

    function requestFramePreview() {
        if (!filePath || filePath.length === 0) {
            resolvedImageUrl = ""
            loadingPreview = false
            return
        }
        loadingPreview = true
        const opts = isVideo ? { at: frameTime, width: 1200 } : { width: 1200 }
        previewReqId = App.preview.request(filePath, "frame", opts)
    }

    onFilePathChanged: {
        resolvedImageUrl = ""
        fallbackPreviewPath = ""
        frameTime = 0
        requestFramePreview()
    }

    onIsVideoChanged: {
        if (filePath.length > 0)
            requestFramePreview()
    }

    Timer {
        id: scrubDebounce
        interval: 120
        repeat: false
        onTriggered: root.requestFramePreview()
    }

    Connections {
        target: App.preview
        function onReady(requestId, kind, files, width, height) {
            if (requestId !== root.previewReqId || kind !== "frame")
                return
            root.loadingPreview = false
            if (width > 0 && height > 0) {
                root.previewWidth = width
                root.previewHeight = height
            }
            const first = files && files.length > 0 ? String(files[0]) : ""
            root.fallbackPreviewPath = first
            if (!root.isVideo && root.isBuiltInRaster(root.filePath))
                root.resolvedImageUrl = root.fileUrl(root.filePath)
            else if (first.length > 0)
                root.resolvedImageUrl = root.fileUrl(first)
        }
        function onFailed(requestId, message) {
            if (requestId !== root.previewReqId)
                return
            root.loadingPreview = false
        }
    }

    function clampRect(x, y, w, h) {
        const mw = Math.max(1, effWidth)
        const mh = Math.max(1, effHeight)
        const cw = Math.max(1, Math.min(mw, Math.round(w)))
        const ch = Math.max(1, Math.min(mh, Math.round(h)))
        const cx = Math.max(0, Math.min(mw - cw, Math.round(x)))
        const cy = Math.max(0, Math.min(mh - ch, Math.round(y)))
        return { x: cx, y: cy, w: cw, h: ch }
    }

    function setCropRect(x, y, w, h) {
        const c = clampRect(x, y, w, h)
        rectX = c.x
        rectY = c.y
        rectW = c.w
        rectH = c.h
        cropChanged(rectX, rectY, rectW, rectH)
    }

    function selectAspect(label, num, den) {
        aspectLabel = label
        if (num <= 0 || den <= 0) {
            aspectRatio = 0
            return
        }
        const ratio = num / den
        aspectRatio = ratio
        const mw = Math.max(1, effWidth)
        const mh = Math.max(1, effHeight)
        let w = mw
        let h = Math.max(1, Math.round(w / ratio))
        if (h > mh) {
            h = mh
            w = Math.max(1, Math.round(h * ratio))
        }
        const x = Math.floor((mw - w) / 2)
        const y = Math.floor((mh - h) / 2)
        setCropRect(x, y, w, h)
    }

    function addRedactRect(x, y, w, h, style) {
        const c = clampRect(x, y, w, h)
        const st = (style === "blur" || style === "pixelate" || style === "black") ? style : currentStyle
        const next = rects.slice()
        next.push({ x: c.x, y: c.y, w: c.w, h: c.h, style: st })
        rects = next
        selectedIndex = next.length - 1
        currentStyle = st
        rectsModified()
    }

    function addDefaultRedactRect() {
        const mw = Math.max(1, effWidth)
        const mh = Math.max(1, effHeight)
        const bw = Math.max(24, Math.round(mw / 4))
        const bh = Math.max(24, Math.round(mh / 4))
        const offset = (rects.length % 5) * Math.max(12, Math.round(Math.min(mw, mh) * 0.04))
        const bx = Math.min(mw - bw, Math.round(mw * 3 / 8) + offset)
        const by = Math.min(mh - bh, Math.round(mh * 3 / 8) + offset)
        addRedactRect(bx, by, bw, bh, currentStyle)
    }

    function selectRect(idx) {
        if (idx < 0 || idx >= rects.length) {
            selectedIndex = rects.length > 0 ? 0 : -1
            return
        }
        selectedIndex = idx
        currentStyle = rects[idx].style || "black"
        rectsModified()
    }

    function updateSelectedRect(x, y, w, h) {
        if (selectedIndex < 0 || selectedIndex >= rects.length)
            return
        const c = clampRect(x, y, w, h)
        const next = rects.slice()
        const cur = next[selectedIndex]
        next[selectedIndex] = { x: c.x, y: c.y, w: c.w, h: c.h, style: cur.style || currentStyle }
        rects = next
        rectsModified()
    }

    function setSelectedStyle(st) {
        if (st !== "black" && st !== "blur" && st !== "pixelate")
            return
        currentStyle = st
        if (selectedIndex >= 0 && selectedIndex < rects.length) {
            const next = rects.slice()
            const cur = next[selectedIndex]
            next[selectedIndex] = { x: cur.x, y: cur.y, w: cur.w, h: cur.h, style: st }
            rects = next
            rectsModified()
        }
    }

    function removeRectAt(idx) {
        if (idx < 0 || idx >= rects.length)
            return
        const next = rects.filter((_, i) => i !== idx)
        rects = next
        if (next.length === 0)
            selectedIndex = -1
        else if (selectedIndex >= next.length)
            selectedIndex = next.length - 1
        else if (selectedIndex > idx)
            selectedIndex = selectedIndex - 1
        if (selectedIndex >= 0 && selectedIndex < next.length)
            currentStyle = next[selectedIndex].style || "black"
        rectsModified()
    }

    function removeSelectedRect() {
        removeRectAt(selectedIndex)
    }

    component ToolChoice: NButton {
        property bool on: false
        compact: true
        kind: on ? "primary" : "secondary"
    }

    ColumnLayout {
        id: mainCol
        anchors.left: parent.left
        anchors.right: parent.right
        spacing: 10

        // Top toolbar: Aspect ratios (crop) OR Redaction style + Add/Delete (redact)
        RowLayout {
            Layout.fillWidth: true
            spacing: 6

            Flow {
                Layout.fillWidth: true
                visible: root.mode === "crop"
                spacing: 6

                Repeater {
                    model: [
                        ["free", qsTr("Free"), 0, 0],
                        ["1:1", "1:1", 1, 1],
                        ["4:3", "4:3", 4, 3],
                        ["3:2", "3:2", 3, 2],
                        ["16:9", "16:9", 16, 9],
                        ["9:16", "9:16", 9, 16]
                    ]
                    delegate: ToolChoice {
                        required property var modelData
                        text: String(modelData[1])
                        on: root.aspectLabel === String(modelData[0])
                        onClicked: root.selectAspect(String(modelData[0]), Number(modelData[2]), Number(modelData[3]))
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true
                visible: root.mode === "redact"
                spacing: 6

                Text {
                    text: qsTr("Style:")
                    font.pixelSize: Theme.sizeSmall
                    font.weight: Font.DemiBold
                    color: Theme.secondaryText
                }

                Repeater {
                    model: [
                        ["black", qsTr("Black")],
                        ["blur", qsTr("Blur")],
                        ["pixelate", qsTr("Pixelate")]
                    ]
                    delegate: ToolChoice {
                        required property var modelData
                        text: String(modelData[1])
                        on: root.currentStyle === String(modelData[0])
                        onClicked: root.setSelectedStyle(String(modelData[0]))
                    }
                }

                Item { Layout.fillWidth: true }

                NButton {
                    compact: true
                    iconName: "list-add"
                    text: qsTr("Add area")
                    onClicked: root.addDefaultRedactRect()
                }

                NButton {
                    compact: true
                    kind: "quiet"
                    iconName: "edit-delete"
                    text: qsTr("Delete")
                    enabled: root.selectedIndex >= 0 && root.selectedIndex < root.rects.length
                    onClicked: root.removeSelectedRect()
                }
            }

            Pill {
                visible: root.mode === "crop" && root.rectW > 0 && root.rectH > 0
                text: root.rectW + " × " + root.rectH + " px"
                tint: Theme.forest
            }
        }

        // Visual viewport container
        Rectangle {
            id: viewport
            Layout.fillWidth: true
            Layout.preferredHeight: root.stageHeight
            radius: Theme.radiusRow
            color: Theme.pick("#e9e4d5", "#141d17")
            border.width: 1
            border.color: Theme.border
            clip: true

            readonly property real pad: 14
            readonly property real availW: Math.max(1, width - pad * 2)
            readonly property real availH: Math.max(1, height - pad * 2)
            readonly property real fitScale: Math.min(availW / Math.max(1, root.effWidth),
                                                      availH / Math.max(1, root.effHeight))
            readonly property real dispW: Math.max(1, root.effWidth * fitScale)
            readonly property real dispH: Math.max(1, root.effHeight * fitScale)

            Item {
                id: stage
                x: Math.round((viewport.width - viewport.dispW) / 2)
                y: Math.round((viewport.height - viewport.dispH) / 2)
                width: viewport.dispW
                height: viewport.dispH

                // Subtle backdrop plate inside image bounds
                Rectangle {
                    anchors.fill: parent
                    color: Theme.pick("#dfd9c8", "#1b261e")
                    border.width: 1
                    border.color: Theme.border

                    Column {
                        anchors.centerIn: parent
                        spacing: 4
                        visible: previewImg.status !== Image.Ready
                        Text {
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: root.loadingPreview ? qsTr("Loading preview…")
                                                      : (root.effWidth + " × " + root.effHeight)
                            font.pixelSize: Theme.sizeSmall
                            font.weight: Font.DemiBold
                            color: Theme.secondaryText
                        }
                        Text {
                            anchors.horizontalCenter: parent.horizontalCenter
                            visible: root.mode === "redact"
                            text: qsTr("Drag anywhere to cover an area")
                            font.pixelSize: Theme.sizeTiny
                            color: Theme.tertiaryText
                        }
                    }
                }

                Image {
                    id: previewImg
                    anchors.fill: parent
                    source: root.resolvedImageUrl
                    fillMode: Image.Stretch
                    autoTransform: true
                    asynchronous: true
                    smooth: true
                    onStatusChanged: {
                        if (status === Image.Error && root.fallbackPreviewPath.length > 0) {
                            const fb = root.fileUrl(root.fallbackPreviewPath)
                            if (root.resolvedImageUrl !== fb)
                                root.resolvedImageUrl = fb
                        }
                    }
                }

                // ---- SINGLE-RECT CROP OVERLAY ----------------------------
                Item {
                    id: cropOverlay
                    anchors.fill: parent
                    visible: root.mode === "crop" && root.rectW > 0 && root.rectH > 0

                    readonly property real bx: root.rectX * viewport.fitScale
                    readonly property real by: root.rectY * viewport.fitScale
                    readonly property real bw: Math.max(2, root.rectW * viewport.fitScale)
                    readonly property real bh: Math.max(2, root.rectH * viewport.fitScale)

                    // 4 outside dimming curtains
                    Rectangle {
                        x: 0; y: 0
                        width: parent.width
                        height: Math.max(0, cropOverlay.by)
                        color: Qt.rgba(0, 0, 0, 0.52)
                    }
                    Rectangle {
                        x: 0
                        y: cropOverlay.by + cropOverlay.bh
                        width: parent.width
                        height: Math.max(0, parent.height - (cropOverlay.by + cropOverlay.bh))
                        color: Qt.rgba(0, 0, 0, 0.52)
                    }
                    Rectangle {
                        x: 0
                        y: cropOverlay.by
                        width: Math.max(0, cropOverlay.bx)
                        height: cropOverlay.bh
                        color: Qt.rgba(0, 0, 0, 0.52)
                    }
                    Rectangle {
                        x: cropOverlay.bx + cropOverlay.bw
                        y: cropOverlay.by
                        width: Math.max(0, parent.width - (cropOverlay.bx + cropOverlay.bw))
                        height: cropOverlay.bh
                        color: Qt.rgba(0, 0, 0, 0.52)
                    }

                    // Active crop frame + rule-of-thirds + 8 handles
                    Item {
                        id: cropBox
                        x: cropOverlay.bx
                        y: cropOverlay.by
                        width: cropOverlay.bw
                        height: cropOverlay.bh

                        // Rule-of-thirds lines
                        Repeater {
                            model: [1, 2]
                            delegate: Rectangle {
                                required property int modelData
                                x: Math.round(cropBox.width * modelData / 3)
                                y: 0
                                width: 1
                                height: cropBox.height
                                color: Qt.rgba(1, 1, 1, 0.42)
                            }
                        }
                        Repeater {
                            model: [1, 2]
                            delegate: Rectangle {
                                required property int modelData
                                x: 0
                                y: Math.round(cropBox.height * modelData / 3)
                                width: cropBox.width
                                height: 1
                                color: Qt.rgba(1, 1, 1, 0.42)
                            }
                        }

                        // Contrast shadow frame + gold frame
                        Rectangle {
                            anchors.fill: parent
                            anchors.margins: -1
                            color: "transparent"
                            border.width: 1
                            border.color: Qt.rgba(0, 0, 0, 0.45)
                        }
                        Rectangle {
                            anchors.fill: parent
                            color: "transparent"
                            border.width: 2
                            border.color: Theme.gold
                        }

                        // 8 handles: nw, n, ne, e, se, s, sw, w
                        Repeater {
                            model: [
                                [0.0, 0.0], [0.5, 0.0], [1.0, 0.0], [1.0, 0.5],
                                [1.0, 1.0], [0.5, 1.0], [0.0, 1.0], [0.0, 0.5]
                            ]
                            delegate: Rectangle {
                                required property var modelData
                                x: cropBox.width * Number(modelData[0]) - width / 2
                                y: cropBox.height * Number(modelData[1]) - height / 2
                                width: 11
                                height: 11
                                radius: 5.5
                                color: "#ffffff"
                                border.width: 2
                                border.color: Theme.goldInk
                            }
                        }
                    }
                }

                // ---- MULTI-RECT REDACT OVERLAY ---------------------------
                Item {
                    id: redactOverlay
                    anchors.fill: parent
                    visible: root.mode === "redact"

                    Repeater {
                        model: root.rects.length
                        delegate: Item {
                            id: rItem
                            required property int index

                            readonly property var entry: index >= 0 && index < root.rects.length
                                                         ? root.rects[index]
                                                         : ({ x: 0, y: 0, w: 0, h: 0, style: "black" })
                            readonly property string styleName: entry.style ? String(entry.style) : "black"
                            readonly property bool selected: index === root.selectedIndex

                            x: Number(entry.x) * viewport.fitScale
                            y: Number(entry.y) * viewport.fitScale
                            width: Math.max(4, Number(entry.w) * viewport.fitScale)
                            height: Math.max(4, Number(entry.h) * viewport.fitScale)
                            z: selected ? 10 : index

                            // Style: "black"
                            Rectangle {
                                anchors.fill: parent
                                visible: rItem.styleName === "black"
                                color: "#0e120e"
                                opacity: 0.94
                            }

                            // Style: "blur" (frosted translucent glass + soft blur bars)
                            Rectangle {
                                anchors.fill: parent
                                visible: rItem.styleName === "blur"
                                color: Theme.pick(Qt.rgba(0.94, 0.96, 0.93, 0.68), Qt.rgba(0.75, 0.82, 0.77, 0.52))
                                clip: true

                                Column {
                                    id: blurStubCol
                                    anchors.centerIn: parent
                                    spacing: 5
                                    width: Math.max(8, (parent ? parent.width : 14) - 14)
                                    Repeater {
                                        model: 3
                                        delegate: Rectangle {
                                            required property int index
                                            width: Math.max(4, blurStubCol.width * (index === 1 ? 0.9 : 0.68))
                                            height: 4
                                            radius: 2
                                            anchors.horizontalCenter: blurStubCol.horizontalCenter
                                            color: Qt.rgba(0.2, 0.28, 0.22, 0.22)
                                        }
                                    }
                                }
                            }

                            // Style: "pixelate" (mosaic checkerboard pattern)
                            Canvas {
                                id: pixelCanvas
                                anchors.fill: parent
                                visible: rItem.styleName === "pixelate"
                                renderTarget: Canvas.Image
                                onWidthChanged: if (visible) requestPaint()
                                onHeightChanged: if (visible) requestPaint()
                                onVisibleChanged: if (visible) requestPaint()
                                onAvailableChanged: if (available && visible) requestPaint()
                                onPaint: {
                                    if (width <= 0 || height <= 0)
                                        return
                                    const ctx = getContext("2d")
                                    if (!ctx)
                                        return
                                    ctx.clearRect(0, 0, width, height)
                                    const cell = 9
                                    for (let py = 0; py < height; py += cell) {
                                        const row = Math.floor(py / cell)
                                        for (let px = 0; px < width; px += cell) {
                                            const col = Math.floor(px / cell)
                                            const even = ((row + col) % 2) === 0
                                            ctx.fillStyle = even ? "rgba(32, 42, 34, 0.80)"
                                                                 : "rgba(215, 220, 206, 0.76)"
                                            ctx.fillRect(px, py, Math.min(cell, width - px), Math.min(cell, height - py))
                                        }
                                    }
                                }
                            }

                            // Outline
                            Rectangle {
                                anchors.fill: parent
                                color: "transparent"
                                border.width: rItem.selected ? 2 : 1
                                border.color: rItem.selected ? Theme.gold : Qt.rgba(1, 1, 1, 0.55)
                            }

                            // Index & style badge
                            Rectangle {
                                x: 4
                                y: 4
                                visible: rItem.width >= 32 && rItem.height >= 20
                                width: Math.min(rItem.width - 8, badgeLabel.implicitWidth + 10)
                                height: 18
                                radius: 5
                                color: rItem.selected ? Theme.gold : Qt.rgba(0.08, 0.1, 0.08, 0.78)

                                Text {
                                    id: badgeLabel
                                    anchors.centerIn: parent
                                    text: rItem.width >= 76
                                          ? ("#" + (rItem.index + 1) + " · " + root.styleTitle(rItem.styleName))
                                          : ("#" + (rItem.index + 1))
                                    elide: Text.ElideRight
                                    width: Math.max(8, parent.width - 6)
                                    horizontalAlignment: Text.AlignHCenter
                                    font.pixelSize: Theme.sizeTiny
                                    font.weight: Font.Bold
                                    color: rItem.selected ? Theme.buttonInk : "#ffffff"
                                }
                            }

                            // 8 resize handles on selected rect
                            Repeater {
                                model: rItem.selected ? [
                                    [0.0, 0.0], [0.5, 0.0], [1.0, 0.0], [1.0, 0.5],
                                    [1.0, 1.0], [0.5, 1.0], [0.0, 1.0], [0.0, 0.5]
                                ] : []
                                delegate: Rectangle {
                                    required property var modelData
                                    x: rItem.width * Number(modelData[0]) - width / 2
                                    y: rItem.height * Number(modelData[1]) - height / 2
                                    width: 10
                                    height: 10
                                    radius: 5
                                    color: "#ffffff"
                                    border.width: 2
                                    border.color: Theme.goldInk
                                }
                            }
                        }
                    }
                }

                // ---- UNIFIED INTERACTIVE POINTER & KEY HANDLER -----------
                MouseArea {
                    id: pointerArea
                    anchors.fill: parent
                    anchors.margins: -8
                    hoverEnabled: true
                    focus: true

                    property string action: "" // "", "move", "draw", "nw","n","ne","e","se","s","sw","w"
                    property real startStageX: 0
                    property real startStageY: 0
                    property var startBox: ({ x: 0, y: 0, w: 0, h: 0 })
                    property bool createdNewRedact: false

                    function toStagePt(mx, my) {
                        return { x: mx - 8, y: my - 8 }
                    }

                    function activeBox() {
                        if (root.mode === "crop")
                            return { x: root.rectX, y: root.rectY, w: root.rectW, h: root.rectH }
                        if (root.selectedIndex >= 0 && root.selectedIndex < root.rects.length)
                            return root.rects[root.selectedIndex]
                        return null
                    }

                    function hitHandle(sx, sy, box) {
                        if (!box || box.w <= 0 || box.h <= 0)
                            return ""
                        const bx = box.x * viewport.fitScale
                        const by = box.y * viewport.fitScale
                        const bw = box.w * viewport.fitScale
                        const bh = box.h * viewport.fitScale
                        const pts = [
                            ["nw", bx, by],
                            ["ne", bx + bw, by],
                            ["se", bx + bw, by + bh],
                            ["sw", bx, by + bh],
                            ["n", bx + bw / 2, by],
                            ["e", bx + bw, by + bh / 2],
                            ["s", bx + bw / 2, by + bh],
                            ["w", bx, by + bh / 2]
                        ]
                        for (let i = 0; i < pts.length; ++i) {
                            const dx = sx - Number(pts[i][1])
                            const dy = sy - Number(pts[i][2])
                            if (dx * dx + dy * dy <= 11 * 11)
                                return String(pts[i][0])
                        }
                        return ""
                    }

                    function insideBox(sx, sy, box) {
                        if (!box || box.w <= 0 || box.h <= 0)
                            return false
                        const bx = box.x * viewport.fitScale
                        const by = box.y * viewport.fitScale
                        const bw = box.w * viewport.fitScale
                        const bh = box.h * viewport.fitScale
                        return sx >= bx && sx <= bx + bw && sy >= by && sy <= by + bh
                    }

                    function cursorForHandle(h) {
                        if (h === "nw" || h === "se")
                            return Qt.SizeFDiagCursor
                        if (h === "ne" || h === "sw")
                            return Qt.SizeBDiagCursor
                        if (h === "n" || h === "s")
                            return Qt.SizeVerCursor
                        if (h === "e" || h === "w")
                            return Qt.SizeHorCursor
                        if (h === "move")
                            return Qt.SizeAllCursor
                        return Qt.CrossCursor
                    }

                    Keys.onPressed: event => {
                        if (root.mode === "redact"
                                && (event.key === Qt.Key_Delete || event.key === Qt.Key_Backspace)
                                && root.selectedIndex >= 0) {
                            root.removeSelectedRect()
                            event.accepted = true
                        }
                    }

                    onPressed: mouse => {
                        forceActiveFocus()
                        const pt = toStagePt(mouse.x, mouse.y)
                        startStageX = Math.max(0, Math.min(stage.width, pt.x))
                        startStageY = Math.max(0, Math.min(stage.height, pt.y))
                        createdNewRedact = false

                        const cur = activeBox()
                        const h = hitHandle(pt.x, pt.y, cur)
                        if (h.length > 0) {
                            action = h
                            startBox = { x: cur.x, y: cur.y, w: cur.w, h: cur.h }
                            cursorShape = cursorForHandle(h)
                            return
                        }

                        if (root.mode === "crop") {
                            if (insideBox(pt.x, pt.y, cur)) {
                                action = "move"
                                startBox = { x: cur.x, y: cur.y, w: cur.w, h: cur.h }
                                cursorShape = Qt.SizeAllCursor
                            } else {
                                action = "draw"
                                cursorShape = Qt.CrossCursor
                            }
                            return
                        }

                        // Multi-rect hit test from top-most down to 0
                        for (let i = root.rects.length - 1; i >= 0; --i) {
                            if (insideBox(pt.x, pt.y, root.rects[i])) {
                                root.selectRect(i)
                                action = "move"
                                const b = root.rects[i]
                                startBox = { x: b.x, y: b.y, w: b.w, h: b.h }
                                cursorShape = Qt.SizeAllCursor
                                return
                            }
                        }

                        action = "draw"
                        cursorShape = Qt.CrossCursor
                    }

                    onPositionChanged: mouse => {
                        const pt = toStagePt(mouse.x, mouse.y)
                        const scale = Math.max(0.0001, viewport.fitScale)
                        const mw = Math.max(1, root.effWidth)
                        const mh = Math.max(1, root.effHeight)
                        const minPx = Math.min(8, mw, mh)

                        if (!pressed || action.length === 0) {
                            const cur = activeBox()
                            const h = hitHandle(pt.x, pt.y, cur)
                            if (h.length > 0) {
                                cursorShape = cursorForHandle(h)
                            } else if (root.mode === "crop" && insideBox(pt.x, pt.y, cur)) {
                                cursorShape = Qt.SizeAllCursor
                            } else if (root.mode === "redact") {
                                let over = false
                                for (let i = root.rects.length - 1; i >= 0; --i) {
                                    if (insideBox(pt.x, pt.y, root.rects[i])) {
                                        over = true
                                        break
                                    }
                                }
                                cursorShape = over ? Qt.SizeAllCursor : Qt.CrossCursor
                            } else {
                                cursorShape = Qt.CrossCursor
                            }
                            return
                        }

                        const clampedX = Math.max(0, Math.min(stage.width, pt.x))
                        const clampedY = Math.max(0, Math.min(stage.height, pt.y))
                        const dxPx = (clampedX - startStageX) / scale
                        const dyPx = (clampedY - startStageY) / scale

                        if (action === "move") {
                            const nx = Math.max(0, Math.min(mw - startBox.w, Math.round(startBox.x + dxPx)))
                            const ny = Math.max(0, Math.min(mh - startBox.h, Math.round(startBox.y + dyPx)))
                            if (root.mode === "crop")
                                root.setCropRect(nx, ny, startBox.w, startBox.h)
                            else
                                root.updateSelectedRect(nx, ny, startBox.w, startBox.h)
                            return
                        }

                        if (action === "draw") {
                            if (Math.abs(clampedX - startStageX) < 3 && Math.abs(clampedY - startStageY) < 3)
                                return
                            const ax = startStageX / scale
                            const ay = startStageY / scale
                            const cx = clampedX / scale
                            const cy = clampedY / scale
                            let x1 = Math.min(ax, cx)
                            let y1 = Math.min(ay, cy)
                            let w = Math.max(minPx, Math.abs(cx - ax))
                            let h = Math.max(minPx, Math.abs(cy - ay))

                            if (root.mode === "crop" && root.aspectRatio > 0) {
                                const r = root.aspectRatio
                                h = Math.max(1, w / r)
                                const maxW = cx >= ax ? (mw - ax) : ax
                                const maxH = cy >= ay ? (mh - ay) : ay
                                const fitW = Math.min(w, maxW, maxH * r)
                                w = Math.max(minPx, fitW)
                                h = Math.max(1, w / r)
                                x1 = cx >= ax ? ax : (ax - w)
                                y1 = cy >= ay ? ay : (ay - h)
                            }

                            if (root.mode === "crop") {
                                root.setCropRect(x1, y1, w, h)
                            } else {
                                if (!createdNewRedact) {
                                    createdNewRedact = true
                                    root.addRedactRect(x1, y1, w, h, root.currentStyle)
                                } else {
                                    root.updateSelectedRect(x1, y1, w, h)
                                }
                            }
                            return
                        }

                        // Resize handle ("nw","n","ne","e","se","s","sw","w")
                        let left = startBox.x
                        let top = startBox.y
                        let right = startBox.x + startBox.w
                        let bottom = startBox.y + startBox.h

                        if (action.indexOf("w") >= 0)
                            left = Math.max(0, Math.min(right - minPx, startBox.x + dxPx))
                        if (action.indexOf("e") >= 0)
                            right = Math.min(mw, Math.max(left + minPx, startBox.x + startBox.w + dxPx))
                        if (action.indexOf("n") >= 0)
                            top = Math.max(0, Math.min(bottom - minPx, startBox.y + dyPx))
                        if (action.indexOf("s") >= 0)
                            bottom = Math.min(mh, Math.max(top + minPx, startBox.y + startBox.h + dyPx))

                        if (root.mode === "crop" && root.aspectRatio > 0) {
                            const r = root.aspectRatio
                            let desiredW = right - left
                            let desiredH = bottom - top
                            if (action === "n" || action === "s")
                                desiredW = desiredH * r
                            else if (action === "e" || action === "w")
                                desiredH = desiredW / r
                            else {
                                if (desiredW / Math.max(1, desiredH) > r)
                                    desiredH = desiredW / r
                                else
                                    desiredW = desiredH * r
                            }

                            const maxAvailW = action.indexOf("w") >= 0 ? right
                                            : (action.indexOf("e") >= 0 ? (mw - left) : mw)
                            const maxAvailH = action.indexOf("n") >= 0 ? bottom
                                            : (action.indexOf("s") >= 0 ? (mh - top) : mh)
                            const capW = Math.min(maxAvailW, maxAvailH * r)
                            const fw = Math.max(minPx, Math.min(capW, desiredW))
                            const fh = Math.max(1, fw / r)

                            if (action.indexOf("w") >= 0)
                                left = right - fw
                            else if (action.indexOf("e") >= 0)
                                right = left + fw
                            else {
                                const midX = (left + right) / 2
                                left = Math.max(0, Math.min(mw - fw, midX - fw / 2))
                                right = left + fw
                            }

                            if (action.indexOf("n") >= 0)
                                top = bottom - fh
                            else if (action.indexOf("s") >= 0)
                                bottom = top + fh
                            else {
                                const midY = (top + bottom) / 2
                                top = Math.max(0, Math.min(mh - fh, midY - fh / 2))
                                bottom = top + fh
                            }
                        }

                        if (root.mode === "crop")
                            root.setCropRect(left, top, right - left, bottom - top)
                        else
                            root.updateSelectedRect(left, top, right - left, bottom - top)
                    }

                    onReleased: {
                        action = ""
                        createdNewRedact = false
                    }
                }
            }
        }

        // Video frame scrubber (for cropVideo / redactVideo)
        RowLayout {
            Layout.fillWidth: true
            visible: root.isVideo && root.duration > 0
            spacing: 10

            Text {
                text: qsTr("Preview frame")
                font.pixelSize: Theme.sizeSmall
                font.weight: Font.DemiBold
                color: Theme.secondaryText
            }

            Slider {
                id: frameSlider
                Layout.fillWidth: true
                from: 0
                to: Math.max(0.1, root.duration)
                value: Math.min(to, Math.max(0, root.frameTime))
                onMoved: {
                    root.frameTime = value
                    scrubDebounce.restart()
                }
            }

            Text {
                text: root.formatTime(root.frameTime) + " / " + root.formatTime(root.duration)
                font.pixelSize: Theme.sizeSmall
                color: Theme.secondaryText
            }
        }

        // Redaction boxes chip selector bar
        Flow {
            Layout.fillWidth: true
            visible: root.mode === "redact" && root.rects.length > 0
            spacing: 6

            Repeater {
                model: root.rects.length
                delegate: Rectangle {
                    id: chip
                    required property int index
                    readonly property var item: index >= 0 && index < root.rects.length
                                                ? root.rects[index]
                                                : ({ x: 0, y: 0, w: 0, h: 0, style: "black" })
                    readonly property bool active: index === root.selectedIndex

                    implicitWidth: chipRow.implicitWidth + 16
                    implicitHeight: 28
                    radius: 9
                    color: active ? Theme.sand : Theme.inset
                    border.width: active ? 1.5 : 1
                    border.color: active ? Theme.gold : Theme.border

                    Row {
                        id: chipRow
                        anchors.centerIn: parent
                        spacing: 6

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: "#" + (chip.index + 1) + " · " + root.styleTitle(String(chip.item.style || "black"))
                            font.pixelSize: Theme.sizeSmall
                            font.weight: chip.active ? Font.Bold : Font.Medium
                            color: Theme.text

                            MouseArea {
                                anchors.fill: parent
                                anchors.margins: -4
                                cursorShape: Qt.PointingHandCursor
                                onClicked: root.selectRect(chip.index)
                            }
                        }

                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: "×"
                            font.pixelSize: Theme.sizeBody
                            font.weight: Font.Bold
                            color: Theme.secondaryText

                            MouseArea {
                                anchors.fill: parent
                                anchors.margins: -4
                                cursorShape: Qt.PointingHandCursor
                                onClicked: root.removeRectAt(chip.index)
                            }
                        }
                    }
                }
            }
        }
    }
}
