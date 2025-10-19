import { useMemo, useState } from "react"
import { Button } from "@/components/ui/button"

type CreateBallotProps = {
  title?: string
  candidates?: Record<number, string>
  maxRank?: number
  onSubmit?: (ranking: number[]) => void
}

const DEFAULT_TITLE = "DAO Council Election — Rank Your Delegates"
const DEFAULT_CANDIDATES: Record<number, string> = {
  0: "Delegate Alice (Core Dev)",
  1: "Delegate Bob (Treasury Guild)",
  2: "Delegate Carol (Risk)",
  3: "Delegate Eve (Security Audit)"
}

const ordinal = (n: number) =>
  n % 10 === 1 && n % 100 !== 11 ? `${n}st`
  : n % 10 === 2 && n % 100 !== 12 ? `${n}nd`
  : n % 10 === 3 && n % 100 !== 13 ? `${n}rd`
  : `${n}th`

export default function CreateBallotBoard({
  title = DEFAULT_TITLE,
  candidates = DEFAULT_CANDIDATES,
  maxRank = Object.keys(DEFAULT_CANDIDATES).length,
  onSubmit,
}: CreateBallotProps) {
  const entries = useMemo(
    () =>
      (Object.entries(candidates) as [string, string][])
        .sort((a, b) => Number(a[0]) - Number(b[0])),
    [candidates]
  )
  const keysNum = useMemo(() => entries.map(([k]) => Number(k)), [entries])
  const names   = useMemo(() => entries.map(([, v]) => v), [entries])

  const [ranks, setRanks] = useState<Array<number | null>>(
    Array(maxRank).fill(null)
  )
  const [submitting, setSubmitting] = useState(false)

  const assignments = useMemo(() => {
    const a = Array(keysNum.length).fill(null) as Array<number | null>
    ranks.forEach((rowIdx, rankIdx) => {
      if (rowIdx !== null) a[rowIdx] = rankIdx
    })
    return a
  }, [ranks, keysNum.length])

  const handleSelect = (rowIdx: number, rankIdx: number) => {
    setRanks(prev => {
      const next = [...prev]
      const oldRankOfCandidate = prev.findIndex(r => r === rowIdx)
      const occupantRowAtTarget = prev[rankIdx]

      if (oldRankOfCandidate === rankIdx) {
        next[rankIdx] = null
        return next
      }

      // A) candidate had old rank & target occupied → SWAP
      if (oldRankOfCandidate !== -1 && occupantRowAtTarget !== null) {
        next[rankIdx] = rowIdx
        next[oldRankOfCandidate] = occupantRowAtTarget
        return next
      }

      // B) candidate had old rank & target empty → MOVE
      if (oldRankOfCandidate !== -1 && occupantRowAtTarget === null) {
        next[rankIdx] = rowIdx
        next[oldRankOfCandidate] = null
        return next
      }

      // C) candidate had no old rank & target occupied → PLACE & RELOCATE EVICTED
      if (oldRankOfCandidate === -1 && occupantRowAtTarget !== null) {
        next[rankIdx] = rowIdx
        // keep “always full once filled” by pushing evicted to first empty slot
        const firstEmpty = next.findIndex(v => v === null)
        if (firstEmpty !== -1) next[firstEmpty] = occupantRowAtTarget
        return next
      }

      // D) candidate had no old rank & target empty → SIMPLE PLACE
      next[rankIdx] = rowIdx
      return next
    })
  }

  const allFilled = ranks.every(r => r !== null)

  // Final ballot: number[] of candidate KEYS (ordered by preference)
  const permNums: number[] = useMemo(() => {
    if (!allFilled) return []
    return ranks.map(rowIdx => keysNum[rowIdx as number])
  }, [ranks, keysNum, allFilled])

  const reset = () => setRanks(Array(maxRank).fill(null))

  const submit = async () => {
    if (!allFilled) return
    setSubmitting(true)
    try {
      onSubmit?.(permNums)
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <div className="w-full max-w-3xl mx-auto">
      <h2 className="text-2xl font-semibold tracking-tight mb-4">{title}</h2>

      <div className="rounded-xl border bg-white shadow-sm overflow-hidden">
        <div
          className="grid text-sm font-medium text-muted-foreground border-b"
          style={{
            gridTemplateColumns: `minmax(200px,1fr) repeat(${maxRank}, minmax(48px,64px))`,
          }}
        >
          <div className="p-3 pl-4">Choices</div>
          {Array.from({ length: maxRank }).map((_, i) => (
            <div key={i} className="p-3 text-center">{ordinal(i + 1)}</div>
          ))}
        </div>

        <div role="grid" aria-label="Ranked choice grid" className="divide-y">
          {names.map((name, rowIdx) => (
            <div
              key={`${keysNum[rowIdx]}:${name}`}
              role="row"
              className="grid items-center"
              style={{
                gridTemplateColumns: `minmax(200px,1fr) repeat(${maxRank}, minmax(48px,64px))`,
              }}
            >
              <div className="p-3 pl-4 text-sm">
                {name} <span className="text-xs text-muted-foreground">(key {keysNum[rowIdx]})</span>
              </div>

              {Array.from({ length: maxRank }).map((_, rankIdx) => {
                const selected = assignments[rowIdx] === rankIdx
                const occupiedBy = ranks[rankIdx]
                const isColumnTakenByOther =
                  occupiedBy !== null && occupiedBy !== rowIdx

                return (
                  <SquareCell
                    key={rankIdx}
                    selected={selected}
                    disabled={false}
                    dim={isColumnTakenByOther && !selected}
                    label={`${name} as ${ordinal(rankIdx + 1)}`}
                    onActivate={() => handleSelect(rowIdx, rankIdx)}
                  />
                )
              })}
            </div>
          ))}
        </div>

        <div className="p-4 border-t space-y-2 text-sm">
          <div className="text-muted-foreground break-all">
            <div className="font-medium mb-1">Permutation (number keys):</div>
            <code>{JSON.stringify(permNums)}</code>
          </div>

          <div className="flex items-center justify-end gap-3 pt-2">
            <Button type="button" variant="outline" onClick={reset} className="min-w-[110px]">
              Reset
            </Button>
            <Button
              type="button"
              onClick={submit}
              disabled={submitting || !allFilled}
              className="min-w-[110px]"
            >
              {submitting ? "Submitting…" : "Vote"}
            </Button>
          </div>
        </div>
      </div>
    </div>
  )
}

function SquareCell({
  selected,
  disabled,
  dim,
  label,
  onActivate,
}: {
  selected: boolean
  disabled?: boolean
  dim?: boolean
  label: string
  onActivate: () => void
}) {
  const base =
    "m-2 aspect-square w-10 min-w-10 max-w-12 rounded-md border transition " +
    "flex items-center justify-center select-none outline-none"
  const state = disabled
    ? "opacity-50 cursor-not-allowed"
    : "cursor-pointer hover:shadow-sm"
  const fill = selected
    ? "bg-blue-600 text-white border-blue-600"
    : "bg-white border-muted-foreground/30"
  const tone = dim && !selected ? "opacity-40" : ""

  return (
    <div
      role="gridcell"
      aria-label={label}
      aria-selected={selected}
      tabIndex={disabled ? -1 : 0}
      onClick={() => !disabled && onActivate()}
      onKeyDown={(e) => {
        if (disabled) return
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault()
          onActivate()
        }
      }}
      className={[base, state, fill, tone].join(" ")}
      title={label}
    >
      <div className={`h-2.5 w-2.5 rounded-full ${selected ? "bg-white" : "bg-transparent"}`} />
    </div>
  )
}
