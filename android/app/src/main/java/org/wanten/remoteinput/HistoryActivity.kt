package org.wanten.remoteinput

import android.content.Intent
import android.os.Bundle
import android.view.View
import android.view.ViewGroup
import android.widget.ArrayAdapter
import android.widget.ImageButton
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.ListView
import android.widget.TextView
import androidx.appcompat.app.AlertDialog
import androidx.appcompat.app.AppCompatActivity
import androidx.appcompat.widget.SwitchCompat
import java.util.Calendar

class HistoryActivity : AppCompatActivity() {

    private lateinit var history: HistoryStore
    private lateinit var adapter: HistoryAdapter
    private lateinit var recordLabel: TextView
    private lateinit var recordSwitch: SwitchCompat
    private lateinit var countBadge: TextView
    private val expandedIds = mutableSetOf<String>()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_history)
        applyContentInsets()
        history = HistoryStore(this)
        recordLabel = findViewById(R.id.record_label)
        recordSwitch = findViewById(R.id.record_switch)
        countBadge = findViewById(R.id.count_badge)
        adapter = HistoryAdapter()
        val list = findViewById<ListView>(R.id.history_list)
        list.emptyView = findViewById(R.id.empty_view)
        list.adapter = adapter
        findViewById<ImageButton>(R.id.back_button).apply { pressScale() }.setOnClickListener {
            finish()
        }
        findViewById<View>(R.id.back_bar_button).apply { pressScale() }.setOnClickListener {
            finish()
        }
        recordSwitch.setOnCheckedChangeListener { _, checked ->
            history.recording = checked
            refresh()
        }
        findViewById<ImageButton>(R.id.clear_unpinned_button).apply { pressScale() }.setOnClickListener {
            confirmClearUnpinned()
        }
        refresh()
    }

    private fun refresh() {
        val recording = history.recording
        recordLabel.setText(if (recording) R.string.recording else R.string.paused)
        recordLabel.setTextColor(getColor(if (recording) R.color.green else R.color.muted))
        if (recordSwitch.isChecked != recording) recordSwitch.isChecked = recording
        val items = history.items()
        countBadge.text = items.size.toString()
        countBadge.visibility = if (items.isEmpty()) View.GONE else View.VISIBLE
        adapter.clear()
        adapter.addAll(items)
    }

    private fun confirmClearUnpinned() {
        val items = history.items()
        val unpinned = items.count { !it.pinned }
        if (unpinned == 0) return
        AlertDialog.Builder(this, R.style.DialogDark)
            .setMessage("Delete all unpinned history? ($unpinned items)")
            .setPositiveButton(R.string.delete) { _, _ ->
                history.clearUnpinned()
                refresh()
            }
            .setNegativeButton(R.string.cancel, null)
            .show()
    }

    private fun confirmDelete(item: HistoryItem) {
        AlertDialog.Builder(this, R.style.DialogDark)
            .setMessage(R.string.delete_confirm)
            .setPositiveButton(R.string.delete) { _, _ ->
                history.delete(item.id)
                refresh()
            }
            .setNegativeButton(R.string.cancel, null)
            .show()
    }

    private fun formatTime(ts: Long): String {
        val cal = Calendar.getInstance()
        val now = Calendar.getInstance()
        cal.timeInMillis = ts
        fun pad(n: Int) = if (n < 10) "0$n" else "$n"
        val time = "${pad(cal.get(Calendar.HOUR_OF_DAY))}:${pad(cal.get(Calendar.MINUTE))}"
        val sameDay = cal.get(Calendar.YEAR) == now.get(Calendar.YEAR) &&
            cal.get(Calendar.DAY_OF_YEAR) == now.get(Calendar.DAY_OF_YEAR)
        if (sameDay) return "Today $time"
        now.add(Calendar.DAY_OF_YEAR, -1)
        val yesterday = cal.get(Calendar.YEAR) == now.get(Calendar.YEAR) &&
            cal.get(Calendar.DAY_OF_YEAR) == now.get(Calendar.DAY_OF_YEAR)
        if (yesterday) return "Yesterday $time"
        return "${cal.get(Calendar.MONTH) + 1}/${cal.get(Calendar.DAY_OF_MONTH)} $time"
    }

    private inner class HistoryAdapter :
        ArrayAdapter<HistoryItem>(
            this@HistoryActivity,
            R.layout.item_history,
            mutableListOf()
        ) {
        override fun getView(position: Int, convertView: View?, parent: ViewGroup): View {
            val view = convertView
                ?: layoutInflater.inflate(R.layout.item_history, parent, false)
            val item = getItem(position) ?: return view
            val needsClamp = item.text.length > 200
            val expanded = expandedIds.contains(item.id)
            val body = view.findViewById<TextView>(R.id.history_text)
            body.text = item.text
            if (needsClamp && !expanded) {
                body.maxLines = 7
            } else {
                body.maxLines = Int.MAX_VALUE
            }
            view.findViewById<View>(R.id.pin_bar).visibility =
                if (item.pinned) View.VISIBLE else View.GONE
            val metaIcon = view.findViewById<ImageView>(R.id.meta_icon)
            if (item.pinned) {
                metaIcon.setImageResource(R.drawable.ic_pin_filled)
                metaIcon.imageTintList = null
            } else {
                metaIcon.setImageResource(R.drawable.ic_history)
                metaIcon.imageTintList = android.content.res.ColorStateList.valueOf(getColor(R.color.muted))
            }
            view.findViewById<TextView>(R.id.meta_text).text =
                "${formatTime(item.time)} · ${item.text.length} chars"
            val buttons = view.findViewById<LinearLayout>(R.id.card_buttons)
            buttons.orientation = if (needsClamp) LinearLayout.VERTICAL else LinearLayout.HORIZONTAL
            val expand = view.findViewById<ImageButton>(R.id.expand_button)
            expand.visibility = if (needsClamp) View.VISIBLE else View.GONE
            expand.setImageResource(
                if (expanded) R.drawable.ic_chevron_up else R.drawable.ic_chevron_down
            )
            expand.setOnClickListener {
                if (expanded) expandedIds.remove(item.id) else expandedIds.add(item.id)
                notifyDataSetChanged()
            }
            val pin = view.findViewById<ImageButton>(R.id.pin_button)
            if (item.pinned) {
                pin.setImageResource(R.drawable.ic_pin_filled)
                pin.imageTintList = null
            } else {
                pin.setImageResource(R.drawable.ic_pin)
                pin.imageTintList = android.content.res.ColorStateList.valueOf(getColor(R.color.muted))
            }
            pin.setOnClickListener {
                history.setPinned(item.id, !item.pinned)
                refresh()
            }
            view.findViewById<ImageButton>(R.id.delete_button).setOnClickListener {
                confirmDelete(item)
            }
            body.setOnClickListener {
                setResult(RESULT_OK, Intent().putExtra(InputActivity.EXTRA_TEXT, item.text))
                finish()
            }
            return view
        }
    }
}
