"""Code descriptions edi-835-parser attaches to the codes it reads.

The tables are copied verbatim from edi-835-parser 1.8.0, by
keiron-stoddart / Senscio Systems
(https://github.com/keironstoddart/edi-835-parser), so the objects of this
package carry the same descriptions; its claim status registry, a list of
``Status`` objects there, is the dict ``STATUSES`` here with the same
entries. They are used under that project's MIT License, reproduced below as
its LICENSE file reads.

edi-835-parser, MIT License:

Copyright (c) 2018 The Python Packaging Authority

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
"""

ADJUSTMENT_GROUPS = {
    "CR": "corrections and reversals",
    "OA": "other adjustment",
    "PR": "patient responsibility",
    "CO": "contractual obligation",
    "PI": "payor initiated reduction",
}
DATE_QUALIFIERS = {
    "050": "received",
    "150": "service period start",
    "151": "service period end",
    "472": "service",
    "232": "claim statement period start",
    "233": "claim statement period end",
}
ENTITY_CODES = {"QC": "patient", "74": "insured", "82": "rendering provider", "85": "billing provider"}
ENTITY_TYPES = {"1": "person", "2": "entity"}
IDENTIFICATION_QUALIFIERS = {
    "MI": "member identification number",
    "C": "insured's changed unique identification number",
    "PC": "provider commercial number",
    "XX": "national provider id",
}
AMOUNT_QUALIFIERS = {"B6": "allowed - actual", "AU": "coverage amount"}
ORGANIZATION_TYPES = {"PE": "payee", "PR": "payer"}
PAYMENT_METHODS = {"ACH": "automatic deposit", "CHK": "check", "NON": "no payment"}
ADJUSTMENT_REASONS = {
    '45': 'Charge exceeds fee schedule maximum allowable or contracted/legislated fee arrangement.',
    '243': 'Services not authorized by network/primary care providers.',
    '29': 'The time limit for filing has expired.',
    '251': 'The attachment/other documentation that was received was incomplete or deficient.',
    '2': 'Coinsurnace Amount.',
    '96': 'Non-covered charge(s). See remark code.',
    '3': 'Co-payment Amount.',
    '16': 'Claim/service lacks information or has submission/billing error(s).',
    'B15': 'This service/procedure requires that a qualifying service/procedure be received and covered. The qualifying other service/procedure has not been received/adjudicated.',
    'A1': 'Claim/Service denied. See remark code.',
    '1': 'Deductible Amount',
    '4': 'The procedure code is inconsistent with the modifier used. Usage: Refer to the 835 Healthcare Policy Identification Segment (loop 2110 Service Payment Information REF), if present.',
    '18': "Exact duplicate claim/service (Use only with Group Code OA except where state workers' compensation regulations requires CO)",
    '23': 'The impact of prior payer(s) adjudication including payments and/or adjustments. (Use only with Group Code OA)',
    '26': 'Expenses incurred prior to coverage.',
    '27': 'Expenses incurred after coverage terminated.',
    '97': 'The benefit for this service is included in the payment/allowance for another service/procedure that has already been adjudicated. Usage: Refer to the 835 Healthcare Policy Identification Segment (loop 2110 Service Payment Information REF), if present.',
    '109': 'Claim/service not covered by this payer/contractor. You must send the claim/service to the correct payer/contractor.',
    '151': 'Payment adjusted because the payer deems the information submitted does not support this many/frequency of services.',
    '234': 'This procedure is not paid separately. At least one Remark Code must be provided (may be comprised of either the NCPDP Reject Reason Code, or Remittance Advice Remark Code that is not an ALERT.)',
    '272': 'Coverage/program guidelines were not met.',
}
REMARK_CODES = {
    'N630': 'Referral not authorized by attending physician.',
    'N650': 'This policy was not in effect for this date of loss. No coverage is available.',
    'M53': 'Missing/incomplete/invalid days or units of service.',
    'M15': 'Separately billed services/tests have been bundled as they are considered components of the same procedure. Separate payment is not allowed.',
    'M80': 'Not covered when performed during the same session/date as a previously processed service for the patient.',
    'M86': 'Service denied because payment already made for same/similar procedure within set time frame.',
    'MA130': 'Your claim contains incomplete and/or invalid information, and no appeal rights are afforded because the claim is unprocessable. Please submit a new claim with the complete/correct information.',
    'N122': 'Add-on code cannot be billed by itself.',
    'N20': 'Service not payable with other service rendered on the same date.',
    'N6': 'Under FEHB law (U.S.C. 8904(b)), we cannot pay more for covered care than the amount Medicare would have allowed if the patient were enrolled in Medicare Part A and/or Medicare Part B.',
    'N640': 'Exceeds number/frequency approved/allowed within time period.',
    'N674': 'Not covered unless a pre-requisite procedure/service has been provided.',
    'N702': 'Decision based on review of previously adjudicated claims or for claims in process for the same/similar type of services.',
    'N781': 'Alert: Patient is a Medicaid/ Qualified Medicare Beneficiary. Review your records for any wrongfully collected deductible. This amount may be billed to a subsequent payer.',
    'N782': 'Alert: Patient is a Medicaid/ Qualified Medicare Beneficiary. Review your records for any wrongfully collected coinsurance. This amount may be billed to a subsequent payer.',
    'N807': 'Payment adjustment based on the Merit-based Incentive Payment System (MIPS).',
}
REFERENCE_QUALIFIERS = {
    "6R": "provider control number",
    "0K": "policy form identifying number",
    "PQ": "payee identification",
    "TJ": "federal taxpayer identification number",
    "LU": "location number",
}
REMARK_QUALIFIERS = {"HE": "claim payment"}
ORGANIZATIONS = {
    "AV09311993": "Availity",
    "ZIRMED": "Zirmed",
}
# Claim status code: (description, payer classification, forwarded to another payer).
STATUSES = {
    "1": ("processed as primary", "primary", False),
    "2": ("processed as secondary", "secondary", False),
    "3": ("processed as tertiary", "tertiary", False),
    "4": ("denial", "unspecified", False),
    "19": ("processed as primary, forwarded to additional payer(s)", "primary", True),
    "20": ("processed as secondary, forwarded to additional payer(s)", "secondary", True),
    "21": ("processed as tertiary, forwarded to additional payer(s)", "tertiary", True),
    "22": ("reversal of previous payment", "unspecified", False),
}
UNKNOWN_STATUS = ("uncategorized", "unknown", False)
