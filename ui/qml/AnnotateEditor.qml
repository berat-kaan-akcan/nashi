pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import NashiApp

// Vector annotation editor over an image: Pen, Highlighter, Line, Arrow, Number, Callout,
// Rectangle, Ellipse, Text, Picture and Magnifier. Renders only annotations onto a transparent
// full-resolution Canvas so exportOverlay() saves a clean alpha PNG.
Item {
    id: root
    objectName: "annotateEditor"

    property string filePath: ""
    property int mediaWidth: 0
    property int mediaHeight: 0

    property string currentTool: "pen" // "pen"|"highlighter"|"line"|"arrow"|"rect"|"ellipse"|"text"
    property string currentColor: "#e03131"
    property string strokePreset: "medium" // "thin"|"medium"|"thick"
    property int opacityPercent: 100
    property bool fillShapes: false
    property string fontFamily: "" // empty keeps the application font
    property int magnifierZoom: 2
    property string stampPath: ""

    // Natural sizes of chosen pictures, keyed by path, to keep their shape on screen.
    property var stampSizes: ({})
    property var items: []
    // Earlier and undone states of `items`. Whole states, because a move changes
    // an item in place rather than adding one.
    property var undoStack: []
    property var redoStack: []
    property int selectedIndex: -1
    property var moveStart: null
    property var draftItem: null
    property string lastExportPath: ""

    // Pending click position for Text tool (in canvas coordinates)
    property real pendingTextX: -1
    property real pendingTextY: -1

    // Preview loading state
    property int previewReqId: -1
    property int previewWidth: 0
    property int previewHeight: 0
    property string resolvedImageUrl: ""
    property string fallbackPreviewPath: ""
    property bool loadingPreview: false

    readonly property int effWidth: mediaWidth > 0 ? mediaWidth
                                  : (previewWidth > 0 ? previewWidth
                                  : (bgImg.implicitWidth > 0 ? bgImg.implicitWidth : 800))
    readonly property int effHeight: mediaHeight > 0 ? mediaHeight
                                   : (previewHeight > 0 ? previewHeight
                                   : (bgImg.implicitHeight > 0 ? bgImg.implicitHeight : 520))

    readonly property real canvasDownscale: Math.min(1.0, 2400.0 / Math.max(1, Math.max(effWidth, effHeight)))
    readonly property int canvasWidth: Math.max(1, Math.round(effWidth * canvasDownscale))
    readonly property int canvasHeight: Math.max(1, Math.round(effHeight * canvasDownscale))
    readonly property real unitScale: Math.max(1.0, Math.min(canvasWidth, canvasHeight) / 540.0)

    readonly property real activeStrokePx: {
        const base = strokePreset === "thin" ? 3 : (strokePreset === "thick" ? 11 : 6)
        return Math.max(2, Math.round(base * unitScale))
    }

    implicitWidth: 680
    implicitHeight: mainCol.implicitHeight

    function fileUrl(path) {
        if (!path || path.length === 0)
            return ""
        return "file://" + String(path).split("/").map(encodeURIComponent).join("/")
    }

    function isBuiltInRaster(path) {
        return /\.(png|jpe?g|bmp)$/i.test(String(path))
    }

    function resetEditor() {
        previewReqId = -1
        previewWidth = 0
        previewHeight = 0
        resolvedImageUrl = ""
        fallbackPreviewPath = ""
        loadingPreview = false
        currentTool = "pen"
        currentColor = "#e03131"
        strokePreset = "medium"
        opacityPercent = 100
        fillShapes = false
        fontFamily = ""
        magnifierZoom = 2
        stampPath = ""
        items = []
        undoStack = []
        redoStack = []
        selectedIndex = -1
        moveStart = null
        draftItem = null
        lastExportPath = ""
        pendingTextX = -1
        pendingTextY = -1
        noteField.text = ""
        annotationCanvas.requestPaint()
        requestFramePreview()
    }

    function requestFramePreview() {
        if (!filePath || filePath.length === 0) {
            resolvedImageUrl = ""
            loadingPreview = false
            return
        }
        loadingPreview = true
        previewReqId = App.preview.request(filePath, "frame", { width: 1600 })
    }

    onFilePathChanged: {
        resolvedImageUrl = ""
        fallbackPreviewPath = ""
        requestFramePreview()
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
            if (root.isBuiltInRaster(root.filePath))
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

    function remember() {
        undoStack = undoStack.concat([items])
        redoStack = []
        lastExportPath = ""
    }

    function pushAction(item) {
        remember()
        items = items.concat([item])
        annotationCanvas.requestPaint()
    }

    function undo() {
        if (undoStack.length === 0)
            return
        redoStack = redoStack.concat([items])
        items = undoStack[undoStack.length - 1]
        undoStack = undoStack.slice(0, -1)
        selectedIndex = -1
        lastExportPath = ""
        annotationCanvas.requestPaint()
    }

    function redo() {
        if (redoStack.length === 0)
            return
        undoStack = undoStack.concat([items])
        items = redoStack[redoStack.length - 1]
        redoStack = redoStack.slice(0, -1)
        selectedIndex = -1
        lastExportPath = ""
        annotationCanvas.requestPaint()
    }

    function nextNumber() {
        let highest = 0
        for (const it of items) {
            if (it.kind === "number")
                highest = Math.max(highest, Number(it.number))
        }
        return highest + 1
    }

    // Canvas coordinates, like the pointer.
    // Local path of the picture on screen, for tools that take pixels from it.
    function backgroundPath() {
        return String(resolvedImageUrl).replace(/^file:\/\//, "").split("/").map(decodeURIComponent).join("/")
    }

    // A drag on the picture for the tool in use, with what that tool draws with.
    function dragItem(x, y) {
        const item = {
            kind: currentTool,
            color: currentColor,
            size: activeStrokePx,
            alpha: opacityPercent / 100,
            x1: x,
            y1: y,
            x2: x,
            y2: y
        }
        if (fillShapes && (currentTool === "rect" || currentTool === "ellipse"))
            item.fill = currentColor
        if (currentTool === "image")
            item.path = stampPath
        if (currentTool === "magnifier") {
            item.path = backgroundPath()
            item.zoom = magnifierZoom
        }
        return item
    }

    function addNumberAt(x, y) {
        pushAction({
            kind: "number",
            color: currentColor,
            size: Math.max(24, Math.round(activeStrokePx * 5)),
            alpha: opacityPercent / 100,
            font: fontFamily,
            number: nextNumber(),
            x: Number(x),
            y: Number(y)
        })
    }

    function addCalloutAt(x, y, text) {
        const t = String(text || "").trim()
        if (t.length === 0)
            return
        pushAction({
            kind: "callout",
            color: currentColor,
            size: Math.max(18, Math.round(activeStrokePx * 3.8)),
            alpha: opacityPercent / 100,
            font: fontFamily,
            x: Number(x),
            y: Number(y),
            text: t
        })
    }

    // The box an item covers. Text widths are estimates, which is close enough to pick by.
    function itemBounds(it) {
        const size = Number(it.size || 4)
        const pad = Math.max(8, size / 2)
        if (it.points) {
            const xs = it.points.map(p => p.x)
            const ys = it.points.map(p => p.y)
            return [Math.min(...xs) - pad, Math.min(...ys) - pad, Math.max(...xs) + pad, Math.max(...ys) + pad]
        }
        if (it.x1 !== undefined) {
            return [Math.min(it.x1, it.x2) - pad, Math.min(it.y1, it.y2) - pad,
                    Math.max(it.x1, it.x2) + pad, Math.max(it.y1, it.y2) + pad]
        }
        if (it.kind === "number")
            return [it.x - size / 2, it.y - size / 2, it.x + size / 2, it.y + size / 2]
        const width = String(it.text || "").length * size * 0.65
        if (it.kind === "callout")
            return [it.x, it.y - size * 2.7, it.x + width + size, it.y]
        return [it.x, it.y - size / 2, it.x + width, it.y + size / 2]
    }

    function selectAt(x, y) {
        selectedIndex = -1
        for (let i = items.length - 1; i >= 0; --i) {
            const b = itemBounds(items[i])
            if (x >= b[0] && x <= b[2] && y >= b[1] && y <= b[3]) {
                selectedIndex = i
                break
            }
        }
        annotationCanvas.requestPaint()
    }

    function beginMove() {
        moveStart = selectedIndex >= 0 ? items : null
    }

    function moveSelectedBy(dx, dy) {
        if (selectedIndex < 0 || selectedIndex >= items.length)
            return
        const moved = Object.assign({}, items[selectedIndex])
        if (moved.points)
            moved.points = moved.points.map(p => ({ x: p.x + dx, y: p.y + dy }))
        for (const key of ["x", "x1", "x2"]) {
            if (moved[key] !== undefined)
                moved[key] += dx
        }
        for (const key of ["y", "y1", "y2"]) {
            if (moved[key] !== undefined)
                moved[key] += dy
        }
        const next = items.slice()
        next[selectedIndex] = moved
        items = next
        annotationCanvas.requestPaint()
    }

    function endMove() {
        if (moveStart && moveStart !== items) {
            undoStack = undoStack.concat([moveStart])
            redoStack = []
            lastExportPath = ""
        }
        moveStart = null
    }

    function removeSelected() {
        if (selectedIndex < 0 || selectedIndex >= items.length)
            return
        remember()
        items = items.filter((_, i) => i !== selectedIndex)
        selectedIndex = -1
        annotationCanvas.requestPaint()
    }

    function clearAll() {
        if (items.length === 0 && !draftItem)
            return
        items = []
        undoStack = []
        redoStack = []
        selectedIndex = -1
        draftItem = null
        lastExportPath = ""
        pendingTextX = -1
        pendingTextY = -1
        annotationCanvas.requestPaint()
    }

    // Programmatic helpers (coordinates in source image pixels)
    function addStroke(points, color, widthPx) {
        if (!points || points.length === 0)
            return
        const s = canvasDownscale
        const mapped = []
        if (typeof points[0] === "number" && points.length >= 2) {
            for (let i = 0; i + 1 < points.length; i += 2)
                mapped.push({ x: Number(points[i]) * s, y: Number(points[i + 1]) * s })
        } else {
            for (let i = 0; i < points.length; ++i) {
                const pt = points[i]
                if (!pt)
                    continue
                const px = pt.x !== undefined ? Number(pt.x) : Number(pt[0])
                const py = pt.y !== undefined ? Number(pt.y) : Number(pt[1])
                if (!isNaN(px) && !isNaN(py))
                    mapped.push({ x: px * s, y: py * s })
            }
        }
        if (mapped.length === 0)
            return
        pushAction({
            kind: "pen",
            color: color && String(color).length > 0 ? String(color) : currentColor,
            size: widthPx && widthPx > 0 ? Number(widthPx) * s : activeStrokePx,
            alpha: 1.0,
            points: mapped
        })
    }

    function addArrow(x1, y1, x2, y2, color, widthPx) {
        const s = canvasDownscale
        pushAction({
            kind: "arrow",
            color: color && String(color).length > 0 ? String(color) : currentColor,
            size: widthPx && widthPx > 0 ? Number(widthPx) * s : activeStrokePx,
            x1: Number(x1) * s,
            y1: Number(y1) * s,
            x2: Number(x2) * s,
            y2: Number(y2) * s
        })
    }

    function addText(x, y, textStr, color, fontPx) {
        const t = String(textStr || "").trim()
        if (t.length === 0)
            return
        const s = canvasDownscale
        pushAction({
            kind: "text",
            color: color && String(color).length > 0 ? String(color) : currentColor,
            size: fontPx && fontPx > 0 ? Number(fontPx) * s : Math.max(18, Math.round(activeStrokePx * 3.6)),
            x: Number(x) * s,
            y: Number(y) * s,
            text: t
        })
    }

    function placePendingText() {
        const t = noteField.text.trim()
        if (t.length === 0)
            return
        const tx = pendingTextX >= 0 ? pendingTextX : Math.round(canvasWidth * 0.5)
        const ty = pendingTextY >= 0 ? pendingTextY : Math.round(canvasHeight * 0.5)
        if (currentTool === "callout") {
            addCalloutAt(tx, ty, t)
        } else {
            pushAction({
                kind: "text",
                color: currentColor,
                size: Math.max(18, Math.round(activeStrokePx * 3.8)),
                alpha: opacityPercent / 100,
                font: fontFamily,
                x: tx,
                y: ty,
                text: t
            })
        }
        pendingTextX = -1
        pendingTextY = -1
    }

    // White on dark colours and dark on light ones, matching the exported picture.
    function inkOn(color) {
        const c = Qt.color(String(color))
        return 0.299 * c.r + 0.587 * c.g + 0.114 * c.b > 0.6 ? "#1d2117" : "#ffffff"
    }

    function fontSpec(it, pixels) {
        const family = String(it.font || "")
        return "bold " + pixels + "px " + (family.length > 0 ? '"' + family + '"' : "sans-serif")
    }

    function paintSingleItem(ctx, it) {
        if (!it || !it.kind)
            return
        ctx.save()
        ctx.globalAlpha = it.alpha !== undefined ? Number(it.alpha) : 1.0
        ctx.strokeStyle = it.color || "#e03131"
        ctx.fillStyle = it.color || "#e03131"
        ctx.lineWidth = Math.max(1.5, Number(it.size || 4))
        ctx.lineCap = "round"
        ctx.lineJoin = "round"

        const kind = String(it.kind)
        if (kind === "pen" || kind === "highlighter") {
            const pts = it.points || []
            if (pts.length === 1) {
                ctx.beginPath()
                ctx.arc(pts[0].x, pts[0].y, ctx.lineWidth / 2, 0, Math.PI * 2)
                ctx.fill()
            } else if (pts.length > 1) {
                ctx.beginPath()
                ctx.moveTo(pts[0].x, pts[0].y)
                for (let i = 1; i < pts.length; ++i)
                    ctx.lineTo(pts[i].x, pts[i].y)
                ctx.stroke()
            }
        } else if (kind === "line") {
            ctx.beginPath()
            ctx.moveTo(it.x1, it.y1)
            ctx.lineTo(it.x2, it.y2)
            ctx.stroke()
        } else if (kind === "arrow") {
            const dx = it.x2 - it.x1
            const dy = it.y2 - it.y1
            const len = Math.sqrt(dx * dx + dy * dy)
            const angle = Math.atan2(dy, dx)
            const headLen = Math.min(Math.max(14, ctx.lineWidth * 3.8), Math.max(14, len * 0.65))
            const wing = Math.PI / 6.2

            const shaftEndX = len > headLen * 0.6 ? (it.x2 - headLen * 0.55 * Math.cos(angle)) : it.x2
            const shaftEndY = len > headLen * 0.6 ? (it.y2 - headLen * 0.55 * Math.sin(angle)) : it.y2

            ctx.beginPath()
            ctx.moveTo(it.x1, it.y1)
            ctx.lineTo(shaftEndX, shaftEndY)
            ctx.stroke()

            ctx.beginPath()
            ctx.moveTo(it.x2, it.y2)
            ctx.lineTo(it.x2 - headLen * Math.cos(angle - wing),
                       it.y2 - headLen * Math.sin(angle - wing))
            ctx.lineTo(it.x2 - headLen * Math.cos(angle + wing),
                       it.y2 - headLen * Math.sin(angle + wing))
            ctx.closePath()
            ctx.fill()
        } else if (kind === "rect") {
            const rx = Math.min(it.x1, it.x2)
            const ry = Math.min(it.y1, it.y2)
            const rw = Math.abs(it.x2 - it.x1)
            const rh = Math.abs(it.y2 - it.y1)
            if (it.fill) {
                ctx.fillStyle = it.fill
                ctx.fillRect(rx, ry, rw, rh)
            }
            ctx.strokeRect(rx, ry, rw, rh)
        } else if (kind === "ellipse") {
            const cx = (it.x1 + it.x2) / 2
            const cy = (it.y1 + it.y2) / 2
            const radX = Math.max(1, Math.abs(it.x2 - it.x1) / 2)
            const radY = Math.max(1, Math.abs(it.y2 - it.y1) / 2)
            ctx.save()
            ctx.translate(cx, cy)
            ctx.scale(radX, radY)
            ctx.beginPath()
            ctx.arc(0, 0, 1, 0, Math.PI * 2)
            ctx.restore()
            if (it.fill) {
                ctx.fillStyle = it.fill
                ctx.fill()
            }
            ctx.stroke()
        } else if (kind === "image" && it.path) {
            const url = fileUrl(it.path)
            if (annotationCanvas.isImageLoaded(url)) {
                const stamp = stampSizes[it.path]
                const bx = Math.min(it.x1, it.x2)
                const by = Math.min(it.y1, it.y2)
                const bw = Math.abs(it.x2 - it.x1)
                const bh = Math.abs(it.y2 - it.y1)
                const scale = stamp ? Math.min(bw / stamp.width, bh / stamp.height) : 1
                const dw = stamp ? stamp.width * scale : bw
                const dh = stamp ? stamp.height * scale : bh
                ctx.drawImage(url, bx + (bw - dw) / 2, by + (bh - dh) / 2, dw, dh)
            } else {
                annotationCanvas.loadImage(url)
            }
        } else if (kind === "magnifier") {
            const lx = Math.min(it.x1, it.x2)
            const ly = Math.min(it.y1, it.y2)
            const lw = Math.max(1, Math.abs(it.x2 - it.x1))
            const lh = Math.max(1, Math.abs(it.y2 - it.y1))
            const zoom = Number(it.zoom || 2)
            ctx.save()
            ctx.translate(lx + lw / 2, ly + lh / 2)
            ctx.scale(lw / 2, lh / 2)
            ctx.beginPath()
            ctx.arc(0, 0, 1, 0, Math.PI * 2)
            ctx.restore()
            ctx.save()
            ctx.clip()
            if (bgImg.status === Image.Ready) {
                const sx = bgImg.implicitWidth / Math.max(1, canvasWidth)
                const sy = bgImg.implicitHeight / Math.max(1, canvasHeight)
                const seenW = lw / zoom
                const seenH = lh / zoom
                ctx.drawImage(bgImg, (lx + lw / 2 - seenW / 2) * sx, (ly + lh / 2 - seenH / 2) * sy,
                              seenW * sx, seenH * sy, lx, ly, lw, lh)
            }
            ctx.restore()
            ctx.stroke()
        } else if (kind === "number") {
            const size = Number(it.size || 40)
            ctx.beginPath()
            ctx.arc(it.x, it.y, size / 2, 0, Math.PI * 2)
            ctx.fill()
            ctx.fillStyle = inkOn(it.color)
            ctx.font = fontSpec(it, Math.round(size * 0.55))
            ctx.textAlign = "center"
            ctx.textBaseline = "middle"
            ctx.fillText(String(it.number), it.x, it.y)
        } else if (kind === "callout" && it.text) {
            const fontPx = Math.max(14, Math.round(Number(it.size || 24)))
            ctx.font = fontSpec(it, fontPx)
            const tail = fontPx * 0.7
            const pad = fontPx * 0.5
            const boxW = ctx.measureText(String(it.text)).width + pad * 2
            const boxH = fontPx + pad * 2
            const top = it.y - tail - boxH
            ctx.beginPath()
            ctx.moveTo(it.x, it.y)
            ctx.lineTo(it.x + fontPx * 0.4, it.y - tail)
            ctx.lineTo(it.x + fontPx, it.y - tail)
            ctx.closePath()
            ctx.fill()
            ctx.beginPath()
            ctx.roundedRect(it.x, top, boxW, boxH, pad, pad)
            ctx.fill()
            ctx.fillStyle = inkOn(it.color)
            ctx.textBaseline = "middle"
            ctx.fillText(String(it.text), it.x + pad, top + boxH / 2)
        } else if (kind === "text" && it.text) {
            const fontPx = Math.max(14, Math.round(Number(it.size || 24)))
            ctx.font = fontSpec(it, fontPx)
            ctx.textBaseline = "middle"
            // Subtle contrast outline so white or dark text stays legible anywhere
            ctx.lineWidth = Math.max(2, Math.round(fontPx * 0.12))
            ctx.strokeStyle = (String(it.color).toLowerCase() === "#1d2117")
                              ? "rgba(255, 255, 255, 0.72)"
                              : "rgba(0, 0, 0, 0.55)"
            ctx.strokeText(String(it.text), it.x, it.y)
            ctx.fillText(String(it.text), it.x, it.y)
        }

        ctx.restore()
    }

    function paintAll(ctx, w, h, includeDraft) {
        ctx.clearRect(0, 0, w, h)
        for (let i = 0; i < items.length; ++i)
            paintSingleItem(ctx, items[i])
        if (includeDraft && draftItem)
            paintSingleItem(ctx, draftItem)
        // On screen only: the export is drawn separately and never sees this.
        if (includeDraft && selectedIndex >= 0 && selectedIndex < items.length) {
            const b = itemBounds(items[selectedIndex])
            ctx.save()
            ctx.strokeStyle = "#2f6fed"
            ctx.lineWidth = Math.max(2, Math.round(unitScale * 2))
            ctx.setLineDash([4, 3])
            ctx.strokeRect(b[0], b[1], b[2] - b[0], b[3] - b[1])
            ctx.restore()
        }
    }

    function exportOverlay() {
        if (!items || items.length === 0)
            return ""
        draftItem = null
        if (canvasWidth <= 0 || canvasHeight <= 0)
            return ""
        const p = App.preview.renderAnnotationOverlay(canvasWidth, canvasHeight, items)
        if (!p || p.length === 0)
            return ""
        lastExportPath = p
        return p
    }

    component ToolBtn: NButton {
        property bool on: false
        compact: true
        kind: on ? "primary" : "secondary"
    }

    Shortcut {
        sequences: [StandardKey.Undo]
        enabled: root.visible && root.items.length > 0
        onActivated: root.undo()
    }
    Shortcut {
        sequences: [StandardKey.Redo, "Ctrl+Shift+Z"]
        enabled: root.visible && root.redoStack.length > 0
        onActivated: root.redo()
    }

    ColumnLayout {
        id: mainCol
        anchors.left: parent.left
        anchors.right: parent.right
        spacing: 10

        // Row 1: Drawing tools
        Flow {
            Layout.fillWidth: true
            spacing: 6

            Repeater {
                model: [
                    ["pen", qsTr("Pen")],
                    ["highlighter", qsTr("Highlighter")],
                    ["line", qsTr("Line")],
                    ["arrow", qsTr("Arrow")],
                    ["rect", qsTr("Rectangle")],
                    ["ellipse", qsTr("Ellipse")],
                    ["text", qsTr("Text")],
                    ["number", qsTr("Number")],
                    ["callout", qsTr("Callout")],
                    ["image", qsTr("Picture")],
                    ["magnifier", qsTr("Magnifier")],
                    ["select", qsTr("Select / Move")]
                ]
                delegate: ToolBtn {
                    required property var modelData
                    objectName: "tool_" + String(modelData[0])
                    text: String(modelData[1])
                    on: root.currentTool === String(modelData[0])
                    onClicked: {
                        root.currentTool = String(modelData[0])
                        if (root.currentTool === "text" || root.currentTool === "callout")
                            noteField.forceActiveFocus()
                        if (root.currentTool === "image" && root.stampPath.length === 0)
                            stampDialog.open()
                    }
                }
            }
        }

        // Row 2: Colour swatches + Stroke width + Undo / Redo / Clear
        RowLayout {
            Layout.fillWidth: true
            spacing: 8

            Row {
                spacing: 6
                Repeater {
                    model: ["#e03131", "#e3b859", "#2f9e44", "#1971c2", "#ffffff", "#1d2117"]
                    delegate: AbstractButton {
                        id: colorSwatch
                        required property string modelData
                        width: 26
                        height: 26
                        Accessible.name: modelData
                        onClicked: root.currentColor = modelData
                        background: Rectangle {
                            radius: 13
                            color: colorSwatch.modelData
                            border.width: root.currentColor === colorSwatch.modelData ? 3 : 1
                            border.color: root.currentColor === colorSwatch.modelData ? Theme.goldInk : Theme.border
                        }
                    }
                }
            }

            Rectangle {
                width: 1
                height: 20
                color: Theme.border
            }

            Row {
                spacing: 4
                Repeater {
                    model: [
                        ["thin", qsTr("Thin")],
                        ["medium", qsTr("Medium")],
                        ["thick", qsTr("Thick")]
                    ]
                    delegate: ToolBtn {
                        required property var modelData
                        text: String(modelData[1])
                        on: root.strokePreset === String(modelData[0])
                        onClicked: root.strokePreset = String(modelData[0])
                    }
                }
            }

            Item { Layout.fillWidth: true }

            NButton {
                objectName: "annotateUndoButton"
                compact: true
                kind: "quiet"
                iconName: "edit-undo"
                text: qsTr("Undo")
                enabled: root.items.length > 0
                onClicked: root.undo()
            }

            NButton {
                objectName: "annotateRedoButton"
                compact: true
                kind: "quiet"
                iconName: "edit-redo"
                text: qsTr("Redo")
                enabled: root.redoStack.length > 0
                onClicked: root.redo()
            }

            NButton {
                objectName: "annotateDeleteButton"
                visible: root.selectedIndex >= 0
                compact: true
                kind: "quiet"
                iconName: "edit-delete"
                text: qsTr("Delete selected")
                onClicked: root.removeSelected()
            }

            NButton {
                objectName: "annotateClearButton"
                compact: true
                kind: "quiet"
                iconName: "edit-clear"
                text: qsTr("Clear")
                enabled: root.items.length > 0
                onClicked: root.clearAll()
            }
        }

        // Row 3: settings that apply to what gets drawn next
        RowLayout {
            Layout.fillWidth: true
            spacing: 8

            Text {
                text: qsTr("Opacity")
                font.pixelSize: Theme.sizeSmall
                color: Theme.text
            }
            Slider {
                objectName: "annotateOpacity"
                Layout.preferredWidth: 120
                from: 10
                to: 100
                stepSize: 5
                value: root.opacityPercent
                Accessible.name: qsTr("Opacity")
                onMoved: root.opacityPercent = Math.round(value)
            }
            Text {
                Layout.preferredWidth: 34
                text: root.opacityPercent + "%"
                font.pixelSize: Theme.sizeSmall
                color: Theme.secondaryText
            }
            ToolBtn {
                objectName: "annotateFill"
                visible: root.currentTool === "rect" || root.currentTool === "ellipse"
                text: qsTr("Fill")
                on: root.fillShapes
                onClicked: root.fillShapes = !root.fillShapes
            }
            ComboBox {
                objectName: "annotateFont"
                visible: root.currentTool === "text" || root.currentTool === "callout" || root.currentTool === "number"
                Layout.preferredWidth: 180
                model: [qsTr("Default font")].concat(Qt.fontFamilies())
                Accessible.name: qsTr("Font")
                onActivated: (index) => root.fontFamily = index === 0 ? "" : String(textAt(index))
            }
            NButton {
                visible: root.currentTool === "image"
                compact: true
                text: root.stampPath.length > 0 ? App.fileName(root.stampPath) : qsTr("Choose picture…")
                onClicked: stampDialog.open()
            }
            Row {
                visible: root.currentTool === "magnifier"
                spacing: 4
                Repeater {
                    model: [2, 3, 4]
                    delegate: ToolBtn {
                        required property int modelData
                        text: modelData + "×"
                        on: root.magnifierZoom === modelData
                        onClicked: root.magnifierZoom = modelData
                    }
                }
            }
            Item { Layout.fillWidth: true }
        }

        // Inline Text Tool input bar
        RowLayout {
            Layout.fillWidth: true
            visible: root.currentTool === "text" || root.currentTool === "callout"
            spacing: 8

            TextField {
                id: noteField
                objectName: "annotateTextInput"
                Layout.fillWidth: true
                selectByMouse: true
                placeholderText: qsTr("Type text here, then click on the image to place it…")
                onAccepted: root.placePendingText()
            }

            NButton {
                compact: true
                kind: "primary"
                text: qsTr("Place")
                enabled: noteField.text.trim().length > 0
                onClicked: root.placePendingText()
            }
        }

        // Main drawing viewport
        Rectangle {
            id: viewport
            Layout.fillWidth: true
            Layout.preferredHeight: 370
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

                Rectangle {
                    anchors.fill: parent
                    color: Theme.pick("#dfd9c8", "#1b261e")
                    border.width: 1
                    border.color: Theme.border

                    Text {
                        anchors.centerIn: parent
                        visible: bgImg.status !== Image.Ready
                        text: root.loadingPreview ? qsTr("Loading image…")
                                                  : (root.effWidth + " × " + root.effHeight)
                        font.pixelSize: Theme.sizeSmall
                        color: Theme.secondaryText
                    }
                }

                // Background photo (NOT part of annotationCanvas!)
                Image {
                    id: bgImg
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

                // Transparent full-resolution annotation Canvas, visually scaled to stage
                Canvas {
                    id: annotationCanvas
                    objectName: "annotationCanvas"
                    x: 0
                    y: 0
                    width: root.canvasWidth
                    height: root.canvasHeight
                    transformOrigin: Item.TopLeft
                    scale: stage.width / Math.max(1, root.canvasWidth)
                    renderTarget: Canvas.Image
                    renderStrategy: Canvas.Immediate

                    onImageLoaded: requestPaint()
                    onAvailableChanged: if (available) requestPaint()
                    onWidthChanged: if (available) requestPaint()
                    onHeightChanged: if (available) requestPaint()

                    onPaint: {
                        if (width <= 0 || height <= 0)
                            return
                        const ctx = getContext("2d")
                        if (!ctx)
                            return
                        root.paintAll(ctx, width, height, true)
                    }
                }

                // Pending text target reticle when user clicked image before typing text
                Rectangle {
                    visible: (root.currentTool === "text" || root.currentTool === "callout") && root.pendingTextX >= 0 && root.pendingTextY >= 0
                    x: (root.pendingTextX / Math.max(1, root.canvasWidth)) * stage.width - 7
                    y: (root.pendingTextY / Math.max(1, root.canvasHeight)) * stage.height - 7
                    width: 14
                    height: 14
                    radius: 7
                    color: "transparent"
                    border.width: 2
                    border.color: Theme.gold
                }

                MouseArea {
                    id: drawMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: root.currentTool === "text" || root.currentTool === "callout" ? Qt.IBeamCursor
                                 : root.currentTool === "select" ? Qt.SizeAllCursor : Qt.CrossCursor
                    property point lastPoint

                    function toCanvas(mx, my) {
                        const cx = Math.max(0, Math.min(root.canvasWidth,
                                    mx * root.canvasWidth / Math.max(1, stage.width)))
                        const cy = Math.max(0, Math.min(root.canvasHeight,
                                    my * root.canvasHeight / Math.max(1, stage.height)))
                        return { x: cx, y: cy }
                    }

                    onPressed: mouse => {
                        const pt = toCanvas(mouse.x, mouse.y)
                        if (root.currentTool === "select") {
                            root.selectAt(pt.x, pt.y)
                            root.beginMove()
                            lastPoint = Qt.point(pt.x, pt.y)
                            return
                        }
                        if (root.currentTool === "number") {
                            root.addNumberAt(pt.x, pt.y)
                            return
                        }
                        if (root.currentTool === "text" || root.currentTool === "callout") {
                            root.pendingTextX = pt.x
                            root.pendingTextY = pt.y
                            if (noteField.text.trim().length > 0)
                                root.placePendingText()
                            else
                                noteField.forceActiveFocus()
                            return
                        }

                        if (root.currentTool === "image" && root.stampPath.length === 0) {
                            stampDialog.open()
                            return
                        }
                        if (root.currentTool === "pen" || root.currentTool === "highlighter") {
                            const isHi = root.currentTool === "highlighter"
                            root.draftItem = {
                                kind: root.currentTool,
                                color: root.currentColor,
                                size: isHi ? Math.round(root.activeStrokePx * 3.2) : root.activeStrokePx,
                                alpha: (isHi ? 0.38 : 1.0) * root.opacityPercent / 100,
                                points: [pt]
                            }
                        } else {
                            root.draftItem = root.dragItem(pt.x, pt.y)
                        }
                        annotationCanvas.requestPaint()
                    }

                    onPositionChanged: mouse => {
                        if (!pressed)
                            return
                        const pt = toCanvas(mouse.x, mouse.y)
                        if (root.currentTool === "select") {
                            root.moveSelectedBy(pt.x - lastPoint.x, pt.y - lastPoint.y)
                            lastPoint = Qt.point(pt.x, pt.y)
                            return
                        }
                        if (!root.draftItem)
                            return
                        const cur = root.draftItem
                        if (cur.kind === "pen" || cur.kind === "highlighter") {
                            const pts = cur.points.slice()
                            const last = pts[pts.length - 1]
                            const dx = pt.x - last.x
                            const dy = pt.y - last.y
                            if (dx * dx + dy * dy < 2.2)
                                return
                            pts.push(pt)
                            root.draftItem = {
                                kind: cur.kind,
                                color: cur.color,
                                size: cur.size,
                                alpha: cur.alpha,
                                points: pts
                            }
                        } else {
                            root.draftItem = Object.assign({}, cur, { x2: pt.x, y2: pt.y })
                        }
                        annotationCanvas.requestPaint()
                    }

                    onReleased: {
                        if (root.currentTool === "select") {
                            root.endMove()
                            return
                        }
                        if (!root.draftItem)
                            return
                        const finished = root.draftItem
                        root.draftItem = null
                        if (finished.kind !== "pen" && finished.kind !== "highlighter") {
                            const dist = Math.hypot(finished.x2 - finished.x1, finished.y2 - finished.y1)
                            if (dist < 2) {
                                annotationCanvas.requestPaint()
                                return
                            }
                        }
                        root.pushAction(finished)
                    }

                    onCanceled: {
                        root.endMove()
                        root.draftItem = null
                        annotationCanvas.requestPaint()
                    }
                }
            }
        }
    }

    FileDialog {
        id: stampDialog
        title: qsTr("Choose a picture to add")
        nameFilters: [qsTr("Images (*.png *.jpg *.jpeg *.webp *.gif *.bmp)")]
        onAccepted: {
            const paths = App.pathsFromUrls([selectedFile])
            if (paths.length > 0)
                root.stampPath = paths[0]
        }
    }

    // Measures the chosen picture, since the Canvas does not report image sizes.
    Image {
        visible: false
        asynchronous: true
        autoTransform: true
        source: root.stampPath.length > 0 ? root.fileUrl(root.stampPath) : ""
        onStatusChanged: {
            if (status === Image.Ready) {
                const sizes = Object.assign({}, root.stampSizes)
                sizes[root.stampPath] = { width: implicitWidth, height: implicitHeight }
                root.stampSizes = sizes
                annotationCanvas.requestPaint()
            }
        }
    }
}
