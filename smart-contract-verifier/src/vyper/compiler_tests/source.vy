beneficiary: public(address)
auctionStart: public(uint256)
auctionEnd: public(uint256)

highestBidder: public(address)
highestBid: public(uint256)

ended: public(bool)

pendingReturns: public(HashMap[address, uint256])

@external
def __init__(_beneficiary: address, _auction_start: uint256, _bidding_time: uint256):
    self.beneficiary = _beneficiary
    self.auctionStart = _auction_start  # auction start time can be in the past, present or future
    self.auctionEnd = self.auctionStart + _bidding_time
    assert block.timestamp < self.auctionEnd # auction end time should be in the future

@external
@payable
def bid():
    assert block.timestamp >= self.auctionStart
    assert block.timestamp < self.auctionEnd
    assert msg.value > self.highestBid
    self.pendingReturns[self.highestBidder] += self.highestBid
    self.highestBidder = msg.sender
    self.highestBid = msg.value

@external
def withdraw():
    pending_amount: uint256 = self.pendingReturns[msg.sender]
    self.pendingReturns[msg.sender] = 0
    send(msg.sender, pending_amount)

@external
def endAuction():
    assert block.timestamp >= self.auctionEnd
    assert not self.ended
    self.ended = True
    send(self.beneficiary, self.highestBid)