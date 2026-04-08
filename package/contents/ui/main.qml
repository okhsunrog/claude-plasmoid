import QtQuick
import QtQuick.Layouts
import org.kde.plasma.plasmoid
import org.kde.plasma.components as PlasmaComponents
import org.kde.kirigami as Kirigami
import org.kde.plasma.claudeplasmoid

PlasmoidItem {
    id: root

    ClaudeUsage {
        id: usage
        Component.onCompleted: refresh()
    }

    Timer {
        interval: 60000
        running: true
        repeat: true
        onTriggered: usage.refresh()
    }

    // Compact: two donuts filling the panel height
    compactRepresentation: MouseArea {
        id: compactRoot
        readonly property int donutSize: height
        implicitWidth: donutSize * 2 + 12
        Layout.preferredWidth: donutSize * 2 + 12
        Layout.minimumWidth: donutSize * 2 + 12
        onClicked: root.expanded = !root.expanded

        Row {
            anchors.centerIn: parent
            spacing: 6

            // 5-hour session window (orange — semantic session color)
            DonutChart {
                size: compactRoot.donutSize
                value: usage.five_hour_util
                color: "#f0883e"
                label: ""
            }

            // 7-day weekly limit (purple — semantic weekly color)
            DonutChart {
                size: compactRoot.donutSize
                value: usage.seven_day_util
                color: "#a78bfa"
                label: ""
            }
        }
    }

    function utilColor(pct) {
        if (pct < 0)   return Kirigami.Theme.disabledTextColor
        if (pct < 50)  return Kirigami.Theme.positiveTextColor
        if (pct < 80)  return Kirigami.Theme.neutralTextColor
        return Kirigami.Theme.negativeTextColor
    }

    function formatReset(iso) {
        if (!iso) return ""
        const now = new Date()
        const d = new Date(iso)
        let diff = Math.max(0, (d - now) / 1000)
        const days = Math.floor(diff / 86400); diff -= days * 86400
        const hours = Math.floor(diff / 3600); diff -= hours * 3600
        const mins = Math.floor(diff / 60)
        let rel = days > 0 ? days + "d " + hours + "h" : (hours > 0 ? hours + "h " + mins + "m" : mins + "m")
        const dd = String(d.getDate()).padStart(2, "0")
        const mm = String(d.getMonth() + 1).padStart(2, "0")
        const HH = String(d.getHours()).padStart(2, "0")
        const MM = String(d.getMinutes()).padStart(2, "0")
        return "Resets in " + rel + " (" + dd + "/" + mm + ", " + HH + ":" + MM + ")"
    }

    // Popup
    fullRepresentation: ColumnLayout {
        spacing: Kirigami.Units.largeSpacing
        implicitWidth: Kirigami.Units.gridUnit * 22

        Kirigami.Heading {
            Layout.alignment: Qt.AlignHCenter
            text: "Claude Usage"
            level: 3
        }

        // ── Setup form ──────────────────────────────────────────────
        ColumnLayout {
            visible: !usage.configured
            Layout.fillWidth: true
            spacing: Kirigami.Units.smallSpacing

            PlasmaComponents.Label {
                Layout.alignment: Qt.AlignHCenter
                text: "Connect to claude-proxy-rs"
                font.bold: true
            }

            PlasmaComponents.TextField {
                id: urlField
                Layout.fillWidth: true
                placeholderText: "http://localhost:3000"
            }
            PlasmaComponents.TextField {
                id: userField
                Layout.fillWidth: true
                placeholderText: "Username"
            }
            PlasmaComponents.TextField {
                id: passField
                Layout.fillWidth: true
                placeholderText: "Password"
                echoMode: TextInput.Password
            }
            PlasmaComponents.Button {
                Layout.alignment: Qt.AlignHCenter
                text: "Save to KWallet"
                onClicked: {
                    usage.save_credentials(urlField.text, userField.text, passField.text)
                    urlField.text = ""
                    userField.text = ""
                    passField.text = ""
                }
            }
        }

        // ── Subscription limit cards ────────────────────────────────
        GridLayout {
            visible: usage.configured
            Layout.fillWidth: true
            columns: 2
            columnSpacing: Kirigami.Units.smallSpacing
            rowSpacing: Kirigami.Units.smallSpacing

            LimitCard {
                Layout.fillWidth: true
                title: "SESSION (5H)"
                util: usage.five_hour_util
                subtitle: formatReset(usage.five_hour_resets_at)
            }
            LimitCard {
                Layout.fillWidth: true
                title: "WEEKLY (ALL)"
                util: usage.seven_day_util
                subtitle: formatReset(usage.seven_day_resets_at)
            }
            LimitCard {
                Layout.fillWidth: true
                title: "WEEKLY (SONNET)"
                util: usage.seven_day_sonnet_util
                subtitle: formatReset(usage.seven_day_sonnet_resets_at)
            }
            LimitCard {
                Layout.fillWidth: true
                visible: usage.extra_usage_enabled
                title: "EXTRA USAGE"
                util: usage.extra_usage_util
                // API returns extra_usage_* in cents, so divide by 100 for USD
                subtitle: "$" + (usage.extra_usage_used / 100).toFixed(2) + " / $" + (usage.extra_usage_limit / 100).toFixed(2) + " spent"
            }
        }

        PlasmaComponents.Label {
            Layout.alignment: Qt.AlignHCenter
            visible: usage.error !== ""
            text: usage.error
            color: Kirigami.Theme.negativeTextColor
            font.pixelSize: 10
            wrapMode: Text.WordWrap
            Layout.maximumWidth: parent.implicitWidth - Kirigami.Units.gridUnit * 2
        }

        // Reconfigure link
        PlasmaComponents.Button {
            visible: usage.configured
            Layout.alignment: Qt.AlignHCenter
            flat: true
            text: "Reconfigure"
            onClicked: usage.clear_credentials()
        }
    }

    component LimitCard: Rectangle {
        id: card
        property string title: ""
        property real util: -1
        property string subtitle: ""

        implicitHeight: col.implicitHeight + Kirigami.Units.smallSpacing * 2
        color: Kirigami.Theme.alternateBackgroundColor
        radius: 4
        border.color: Kirigami.Theme.separatorColor
        border.width: 1

        ColumnLayout {
            id: col
            anchors.fill: parent
            anchors.margins: Kirigami.Units.smallSpacing
            spacing: 2

            PlasmaComponents.Label {
                text: card.title
                font: Kirigami.Theme.smallFont
                font.bold: true
                color: Kirigami.Theme.disabledTextColor
            }
            PlasmaComponents.Label {
                text: card.util >= 0 ? Math.round(card.util) + "%" : "—"
                font.pointSize: Kirigami.Theme.defaultFont.pointSize * 2
                font.bold: true
                color: utilColor(card.util)
            }
            Rectangle {
                Layout.fillWidth: true
                height: 3
                radius: 1
                color: Kirigami.Theme.separatorColor
                Rectangle {
                    width: parent.width * Math.max(0, Math.min(100, card.util)) / 100
                    height: parent.height
                    radius: 1
                    color: utilColor(card.util)
                }
            }
            PlasmaComponents.Label {
                text: card.subtitle
                font: Kirigami.Theme.smallFont
                color: Kirigami.Theme.disabledTextColor
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
        }
    }

    component DonutChart: Item {
        id: donut
        property real value: -1
        property color color: "#ffffff"
        property string label: ""
        property real size: 100
        property color ringColor: Kirigami.Theme.separatorColor
        property color unknownColor: Kirigami.Theme.disabledTextColor

        width: size
        height: label !== "" ? size + Kirigami.Units.gridUnit * 1.5 : size

        Canvas {
            id: canvas
            width: donut.size
            height: donut.size
            anchors.horizontalCenter: parent.horizontalCenter

            onPaint: {
                const ctx = getContext("2d")
                const cx = width / 2, cy = height / 2
                const r = width / 2 - 2
                const thick = r * 0.28

                ctx.clearRect(0, 0, width, height)

                // Background ring
                ctx.beginPath()
                ctx.arc(cx, cy, r, 0, 2 * Math.PI)
                ctx.strokeStyle = donut.ringColor
                ctx.lineWidth = thick
                ctx.stroke()

                // Value arc
                if (donut.value >= 0) {
                    const end = -Math.PI / 2 + (donut.value / 100) * 2 * Math.PI
                    ctx.beginPath()
                    ctx.arc(cx, cy, r, -Math.PI / 2, end)
                    ctx.strokeStyle = donut.color
                    ctx.lineWidth = thick
                    ctx.lineCap = "round"
                    ctx.stroke()
                }

                // Center text
                ctx.fillStyle = donut.value >= 0 ? donut.color : donut.unknownColor
                ctx.font = "bold " + Math.round(width * 0.32) + "px sans-serif"
                ctx.textAlign = "center"
                ctx.textBaseline = "middle"
                ctx.fillText(donut.value >= 0 ? Math.round(donut.value) + "%" : "?", cx, cy)
            }

            Connections {
                target: donut
                function onValueChanged() { canvas.requestPaint() }
                function onRingColorChanged() { canvas.requestPaint() }
                function onUnknownColorChanged() { canvas.requestPaint() }
            }

            Component.onCompleted: requestPaint()
        }

        PlasmaComponents.Label {
            anchors.top: canvas.bottom
            anchors.topMargin: 4
            anchors.horizontalCenter: parent.horizontalCenter
            text: donut.label
            font.pixelSize: 11
            color: Kirigami.Theme.disabledTextColor
        }
    }
}
